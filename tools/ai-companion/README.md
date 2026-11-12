# 4C Insight AI Companion (A2 prototype)

This is a **developer build**, not a user-ready Firefox extension. It replaces the always-on Python HTTP bridge with **per-request native messaging**. There is no port and no access token.

- `native_host.py` is the Python native messaging host.
- `extension-firefox/` is the unsigned extension source.
- `test_native_host.py` validates strict allowlisted operations.
- The separately generated `.deb` can register the host on ordinary Debian-based Linux. Do not distribute unsigned packages as trusted production installers.

Production requires extension signing, an approved native package installer, real JCode smoke tests, Windows and macOS packages, and reviews of JCode's existing global hooks.
