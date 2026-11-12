import io
import json
import os
from pathlib import Path
import struct
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0,str(Path(__file__).resolve().parent))
import native_host as host

class NativeHostTests(unittest.TestCase):
    def test_bounded_native_message(self):
        payload={'version':2,'op':'status','payload':{}}
        data=json.dumps(payload).encode()
        obj=host.read_message(io.BytesIO(struct.pack('=I',len(data))+data))
        self.assertEqual(obj,payload)
        with self.assertRaisesRegex(ValueError,'length'):
            host.read_message(io.BytesIO(struct.pack('=I',host.MAX_REQUEST_BYTES+1)))
        out=io.BytesIO();host.write_message({'ok':True,'result':{}},out)
        self.assertEqual(struct.unpack('=I',out.getvalue()[:4])[0],len(out.getvalue())-4)
    def test_only_fixed_operations_and_provider(self):
        for x in [{'version':2,'op':'shell','payload':{}},{'version':2,'op':'chat','payload':{'command':'rm -rf'}}]:
            with self.assertRaises((ValueError,RuntimeError)):
                with patch.object(host,'find_jcode',return_value='/usr/bin/jcode'):
                    host.handle(x)
        with self.assertRaisesRegex(ValueError,'provider'):
            host.validated_payload({'provider':'bash','message':'hello'})
    def test_aggregate_context_only(self):
        p={'provider':'openai','message':'hello','context':{
            'context_type':'aggregate_4c_insight_summary_v1','integrative_result_available':True,
            'compare_groups_result_available':False,'pathway_result_available':True,
            'pathway_sets_tested':10,'pathway_evidence_type':'exploratory'}}
        args,_=host.validated_payload(p)
        self.assertEqual(args[:4],['--tool-profile','none','--provider','openai'])
        self.assertIn('aggregate',args[-1].lower())
        p['context']['patient_id']='private'
        with self.assertRaisesRegex(ValueError,'aggregate'):
            host.validated_payload(p)
    def test_no_shell_and_no_source_workspace(self):
        with patch.object(host.subprocess,'run') as run:
            run.return_value.returncode=0
            run.return_value.stdout='Done\n'
            run.return_value.stderr='secret'
            result=host.execute_chat({'provider':'claude','message':'Hi'},'/tmp/jcode')
        self.assertEqual(result['answer'],'Done')
        args,kw=run.call_args
        self.assertEqual(args[0][:5],['/tmp/jcode','--tool-profile','none','--provider','claude'])
        self.assertFalse(kw.get('shell',False))
        self.assertTrue('4c-ai-request-' in kw['cwd'])
    def test_status_without_jcode(self):
        with patch.object(host,'find_jcode',return_value=None):
            self.assertEqual(host.handle({'version':2,'op':'status','payload':{}})['installed'],False)
    def test_provider_login_is_allowlisted_and_errors_if_no_terminal(self):
        with self.assertRaisesRegex(ValueError,'provider'):
            host.open_login('shell','/tmp/jcode','linux')
        with patch.object(host.shutil,'which',return_value=None):
            with self.assertRaisesRegex(RuntimeError,'Run: jcode login'):
                host.open_login('openai','/tmp/jcode','linux')

if __name__=='__main__':unittest.main()
