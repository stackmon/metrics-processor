//! Zitadel machine user identity. The OAuth flow (discovery, assertion signing, token request) is
//! delegated to the `zitadel` crate; only `type: "serviceaccount"` key files are supported.

use anyhow::Context;
use serde::Deserialize;
use zitadel::credentials::{AuthenticationOptions, ServiceAccount};

use crate::config::OidcIdentity;

const SUPPORTED_KEY_TYPE: &str = "serviceaccount";

#[derive(Deserialize)]
struct KeyFileJson {
    #[serde(rename = "type")]
    key_type: Option<String>,
    #[serde(rename = "keyId")]
    key_id: Option<String>,
    key: Option<String>,
    #[serde(rename = "userId")]
    user_id: Option<String>,
}

pub fn load_service_account(path: &str) -> anyhow::Result<ServiceAccount> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("the key file {} cannot be read", path))?;

    validate_key_file(&content)?;

    ServiceAccount::load_from_json(&content)
        .map_err(|err| anyhow::anyhow!("{err}"))
        .context("the key file is not a valid Zitadel service account key file")
}

/// The crate only rejects malformed JSON, so the credential fields it ignores are validated here
/// to keep startup fail-closed.
fn validate_key_file(json: &str) -> anyhow::Result<()> {
    let file: KeyFileJson = serde_json::from_str(json).context("the key file is not valid JSON")?;

    match file.key_type.as_deref() {
        Some(SUPPORTED_KEY_TYPE) => {}
        Some(other) => anyhow::bail!(
            "the key file has type \"{}\", only \"{}\" keys of a Zitadel machine user are supported",
            other,
            SUPPORTED_KEY_TYPE
        ),
        None => anyhow::bail!(
            "the key file has no type, expected \"{}\"",
            SUPPORTED_KEY_TYPE
        ),
    }

    required_field(file.key_id, "keyId")?;
    required_field(file.key, "key")?;
    required_field(file.user_id, "userId")?;

    Ok(())
}

fn required_field(value: Option<String>, name: &str) -> anyhow::Result<String> {
    match value {
        Some(value) if !value.trim().is_empty() => Ok(value),
        Some(_) => anyhow::bail!("the key file has an empty {}", name),
        None => anyhow::bail!("the key file has no {}", name),
    }
}

impl OidcIdentity {
    /// Fixed Zitadel token endpoint, referenced in error messages only.
    pub fn token_url(&self) -> String {
        format!("{}/oauth/v2/token", self.issuer)
    }
}

/// The crate rejects a configured issuer that does not match the discovered one, which fails when
/// the URL keeps a trailing slash.
pub fn normalize_issuer(issuer: &str) -> String {
    issuer.trim_end_matches('/').to_string()
}

pub async fn fetch_service_token(identity: &OidcIdentity) -> anyhow::Result<String> {
    let options = AuthenticationOptions {
        scopes: identity.scopes.clone(),
        ..Default::default()
    };

    let access_token = identity
        .service_account
        .authenticate_with_options(&identity.issuer, &options)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))
        .with_context(|| {
            format!(
                "Failed to request a Zitadel service token from {}",
                identity.token_url()
            )
        })?;

    if access_token.trim().is_empty() {
        anyhow::bail!(
            "Zitadel service token response from {} has no access_token",
            identity.token_url()
        );
    }

    Ok(access_token)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::oidc::test_keys::{
        application_key_file_json, key_file_json_with_type, service_account_key_file_json, USER_ID,
    };
    use tempfile::Builder;

    #[test]
    fn test_load_service_account_key_file() {
        let file = Builder::new().suffix(".json").tempfile().unwrap();
        std::fs::write(file.path(), service_account_key_file_json()).unwrap();

        let service_account = load_service_account(file.path().to_str().unwrap()).unwrap();
        assert!(format!("{:?}", service_account).contains(USER_ID));
    }

    #[test]
    fn test_load_service_account_rejects_unusable_key_files() {
        let dir = Builder::new().tempdir().unwrap();

        let invalid_json = dir.path().join("invalid.json");
        std::fs::write(&invalid_json, "{ not json }").unwrap();

        let unknown_type = dir.path().join("unknown-type.json");
        std::fs::write(&unknown_type, key_file_json_with_type("widget")).unwrap();

        let application_type = dir.path().join("application.json");
        std::fs::write(&application_type, application_key_file_json()).unwrap();

        let empty_key = dir.path().join("empty-key.json");
        std::fs::write(
            &empty_key,
            serde_json::json!({
                "type": SUPPORTED_KEY_TYPE,
                "keyId": "1234",
                "key": "",
                "userId": USER_ID,
            })
            .to_string(),
        )
        .unwrap();

        let cases = [
            dir.path().join("does-not-exist.json"),
            invalid_json,
            unknown_type,
            application_type,
            empty_key,
        ];
        let mut messages = Vec::new();

        for path in cases {
            let message = format!(
                "{:#}",
                load_service_account(path.to_str().unwrap()).unwrap_err()
            );
            messages.push(message);
        }

        assert!(
            messages[0].contains("cannot be read"),
            "unexpected error: {}",
            messages[0]
        );
        assert!(
            messages[1].contains("not valid JSON"),
            "unexpected error: {}",
            messages[1]
        );
        for index in [2, 3] {
            assert!(
                messages[index].contains(SUPPORTED_KEY_TYPE),
                "unexpected error: {}",
                messages[index]
            );
        }
        assert!(
            messages[4].contains("empty key"),
            "unexpected error: {}",
            messages[4]
        );

        for (index, message) in messages.iter().enumerate() {
            assert!(
                !messages[..index].contains(message),
                "error messages are not distinguishable: {}",
                message
            );
        }
    }

    #[test]
    fn test_normalize_issuer() {
        assert_eq!(
            normalize_issuer("https://zitadel.example.com/"),
            "https://zitadel.example.com"
        );
        assert_eq!(
            normalize_issuer("https://zitadel.example.com"),
            "https://zitadel.example.com"
        );
    }
}

#[cfg(test)]
pub(crate) mod test_keys {
    use super::SUPPORTED_KEY_TYPE;
    use rsa::pkcs1::EncodeRsaPrivateKey;
    use rsa::pkcs8::LineEnding;
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
                .to_pkcs1_pem(LineEnding::LF)
                .expect("failed to encode a PKCS#1 test key")
                .to_string()
        })
    }

    pub fn service_account_key_file_json() -> String {
        key_file_json(
            Some(SUPPORTED_KEY_TYPE),
            KEY_ID,
            private_key_pem(),
            Some(("userId", USER_ID)),
        )
    }

    pub fn application_key_file_json() -> String {
        key_file_json(
            Some("application"),
            KEY_ID,
            private_key_pem(),
            Some(("clientId", CLIENT_ID)),
        )
    }

    pub fn key_file_json_with_type(key_type: &str) -> String {
        key_file_json(
            Some(key_type),
            KEY_ID,
            private_key_pem(),
            Some(("userId", USER_ID)),
        )
    }

    fn key_file_json(
        key_type: Option<&str>,
        key_id: &str,
        key: &str,
        id: Option<(&str, &str)>,
    ) -> String {
        let mut file = serde_json::Map::new();
        if let Some(key_type) = key_type {
            file.insert(
                "type".to_string(),
                serde_json::Value::String(key_type.to_string()),
            );
        }
        file.insert(
            "keyId".to_string(),
            serde_json::Value::String(key_id.to_string()),
        );
        file.insert(
            "key".to_string(),
            serde_json::Value::String(key.to_string()),
        );
        if let Some((name, value)) = id {
            file.insert(
                name.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }

        serde_json::Value::Object(file).to_string()
    }
}
