//! Zitadel key file fixtures: the RSA key pair is generated once per test binary in the PKCS#1 PEM
//! format the Zitadel Console exports.

use rsa::pkcs1::EncodeRsaPrivateKey;
use std::path::Path;
use std::sync::OnceLock;

pub const KEY_ID: &str = "81693565968962154";
pub const USER_ID: &str = "392040635458125910";
pub const CLIENT_ID: &str = "392040635458125999";

fn private_key_pem() -> &'static str {
    static PEM: OnceLock<String> = OnceLock::new();

    PEM.get_or_init(|| {
        let private_key = rsa::RsaPrivateKey::new(&mut rand::thread_rng(), 2048)
            .expect("failed to generate a test RSA key");

        private_key
            .to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)
            .expect("failed to encode a PKCS#1 test key")
            .to_string()
    })
}

pub fn public_key_pem() -> String {
    static PEM: OnceLock<String> = OnceLock::new();

    PEM.get_or_init(|| {
        use rsa::pkcs1::DecodeRsaPrivateKey;
        use rsa::pkcs8::{EncodePublicKey, LineEnding};

        let private_key = rsa::RsaPrivateKey::from_pkcs1_pem(private_key_pem())
            .expect("the test key must be a PKCS#1 PEM private key");

        private_key
            .to_public_key()
            .to_public_key_pem(LineEnding::LF)
            .expect("failed to encode the test public key")
    })
    .clone()
}

pub fn service_account_key_file_json() -> String {
    static KEY_FILE: OnceLock<String> = OnceLock::new();

    KEY_FILE
        .get_or_init(|| {
            serde_json::json!({
                "type": "serviceaccount",
                "keyId": KEY_ID,
                "key": private_key_pem(),
                "userId": USER_ID,
            })
            .to_string()
        })
        .clone()
}

pub fn application_key_file_json() -> String {
    static KEY_FILE: OnceLock<String> = OnceLock::new();

    KEY_FILE
        .get_or_init(|| {
            serde_json::json!({
                "type": "application",
                "keyId": KEY_ID,
                "key": private_key_pem(),
                "clientId": CLIENT_ID,
            })
            .to_string()
        })
        .clone()
}

fn write_key_file(path: &Path, content: String) -> String {
    std::fs::write(path, content)
        .unwrap_or_else(|err| panic!("failed to write {}: {}", path.display(), err));

    path.to_str()
        .expect("the key file path is not valid UTF-8")
        .to_string()
}

pub fn write_service_account_key_file(path: &Path) -> String {
    write_key_file(path, service_account_key_file_json())
}

pub fn write_application_key_file(path: &Path) -> String {
    write_key_file(path, application_key_file_json())
}
