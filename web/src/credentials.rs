//! Client-side encrypted project access-key files.
//!
//! The plaintext recovery token never leaves the browser except in the normal
//! Authorization header sent to the same-origin cfs-server.  Exported key files
//! are encrypted locally with PBKDF2-HMAC-SHA-256 + AES-256-GCM using the
//! passphrase supplied by the user.  The server never receives that passphrase.

use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::JsFuture;

const CREDENTIAL_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCredentialPayload {
    pub schema_version: u32,
    pub project_id: String,
    pub recovery_token: String,
    pub display_name: String,
    pub origin: String,
    pub created_unix_seconds: u64,
}

impl ProjectCredentialPayload {
    pub fn new(
        project_id: String,
        recovery_token: String,
        display_name: String,
        origin: String,
        created_unix_seconds: u64,
    ) -> Self {
        Self {
            schema_version: CREDENTIAL_SCHEMA_VERSION,
            project_id,
            recovery_token,
            display_name,
            origin,
            created_unix_seconds,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != CREDENTIAL_SCHEMA_VERSION {
            return Err(format!(
                "unsupported 4C project-key schema version {}",
                self.schema_version
            ));
        }
        if self.project_id.trim().is_empty() || self.recovery_token.trim().is_empty() {
            return Err("encrypted project key does not contain usable credentials".to_string());
        }
        Ok(())
    }
}

#[wasm_bindgen(inline_js = r#"
const CFS_KEY_AAD = new TextEncoder().encode("4C Cohort Explorer encrypted project key v1");

function cfsBytesToBase64(bytes) {
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, Math.min(bytes.length, i + chunk)));
  }
  return btoa(binary);
}

function cfsBase64ToBytes(text) {
  const binary = atob(text);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
  return out;
}

async function cfsDeriveProjectKey(passphrase, salt, iterations) {
  const keyMaterial = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(passphrase),
    { name: "PBKDF2" },
    false,
    ["deriveKey"]
  );
  return crypto.subtle.deriveKey(
    { name: "PBKDF2", salt, iterations, hash: "SHA-256" },
    keyMaterial,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"]
  );
}

export async function cfsEncryptProjectCredential(plaintext, passphrase) {
  if (!globalThis.crypto || !crypto.subtle) throw new Error("WebCrypto is unavailable in this browser");
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const iterations = 310000;
  const key = await cfsDeriveProjectKey(passphrase, salt, iterations);
  const ciphertext = new Uint8Array(await crypto.subtle.encrypt(
    { name: "AES-GCM", iv, additionalData: CFS_KEY_AAD, tagLength: 128 },
    key,
    new TextEncoder().encode(plaintext)
  ));
  return JSON.stringify({
    format: "4c-cohort-explorer-project-key",
    schema_version: 1,
    cipher: "AES-256-GCM",
    kdf: "PBKDF2-HMAC-SHA-256",
    iterations,
    salt_b64: cfsBytesToBase64(salt),
    iv_b64: cfsBytesToBase64(iv),
    ciphertext_b64: cfsBytesToBase64(ciphertext)
  });
}

export async function cfsDecryptProjectCredential(envelopeText, passphrase) {
  if (!globalThis.crypto || !crypto.subtle) throw new Error("WebCrypto is unavailable in this browser");
  let envelope;
  try { envelope = JSON.parse(envelopeText); }
  catch (_) { throw new Error("project key is not valid JSON"); }
  if (envelope.format !== "4c-cohort-explorer-project-key" || envelope.schema_version !== 1) {
    throw new Error("unsupported 4C project-key format");
  }
  if (envelope.cipher !== "AES-256-GCM" || envelope.kdf !== "PBKDF2-HMAC-SHA-256") {
    throw new Error("unsupported 4C project-key encryption parameters");
  }
  const iterations = Number(envelope.iterations);
  if (!Number.isInteger(iterations) || iterations < 100000 || iterations > 2000000) {
    throw new Error("invalid project-key KDF iteration count");
  }
  const salt = cfsBase64ToBytes(envelope.salt_b64);
  const iv = cfsBase64ToBytes(envelope.iv_b64);
  const ciphertext = cfsBase64ToBytes(envelope.ciphertext_b64);
  if (salt.length !== 16 || iv.length !== 12 || ciphertext.length < 16) {
    throw new Error("invalid project-key envelope lengths");
  }
  const key = await cfsDeriveProjectKey(passphrase, salt, iterations);
  let plaintext;
  try {
    plaintext = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv, additionalData: CFS_KEY_AAD, tagLength: 128 },
      key,
      ciphertext
    );
  } catch (_) {
    throw new Error("project-key passphrase is incorrect or the file was modified");
  }
  return new TextDecoder().decode(plaintext);
}

export function cfsCurrentOrigin() {
  return globalThis.location && globalThis.location.origin ? globalThis.location.origin : "";
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = cfsEncryptProjectCredential)]
    fn js_encrypt_project_credential(plaintext: &str, passphrase: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = cfsDecryptProjectCredential)]
    fn js_decrypt_project_credential(envelope_text: &str, passphrase: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = cfsCurrentOrigin)]
    fn js_current_origin() -> String;
}

pub fn current_origin() -> String {
    js_current_origin()
}

pub async fn encrypt(
    payload: &ProjectCredentialPayload,
    passphrase: &str,
) -> Result<Vec<u8>, String> {
    payload.validate()?;
    if passphrase.chars().count() < 12 {
        return Err("Use a project-key passphrase of at least 12 characters.".to_string());
    }
    let plaintext = serde_json::to_string(payload)
        .map_err(|error| format!("could not serialize project credentials: {error}"))?;
    let value = JsFuture::from(js_encrypt_project_credential(&plaintext, passphrase))
        .await
        .map_err(js_error_string)?;
    let envelope = value
        .as_string()
        .ok_or_else(|| "browser encryption returned an invalid project-key envelope".to_string())?;
    Ok(envelope.into_bytes())
}

pub async fn decrypt(bytes: &[u8], passphrase: &str) -> Result<ProjectCredentialPayload, String> {
    if passphrase.is_empty() {
        return Err(
            "Enter the project-key passphrase before loading the encrypted file.".to_string(),
        );
    }
    let envelope = std::str::from_utf8(bytes)
        .map_err(|_| "project-key file is not valid UTF-8 JSON".to_string())?;
    let value = JsFuture::from(js_decrypt_project_credential(envelope, passphrase))
        .await
        .map_err(js_error_string)?;
    let plaintext = value
        .as_string()
        .ok_or_else(|| "browser decryption returned invalid plaintext".to_string())?;
    let payload: ProjectCredentialPayload = serde_json::from_str(&plaintext)
        .map_err(|error| format!("decrypted project-key payload is invalid: {error}"))?;
    payload.validate()?;
    Ok(payload)
}

fn js_error_string(value: JsValue) -> String {
    value
        .dyn_ref::<js_sys::Error>()
        .map(|error| error.message().into())
        .or_else(|| value.as_string())
        .unwrap_or_else(|| "browser cryptography operation failed".to_string())
}
