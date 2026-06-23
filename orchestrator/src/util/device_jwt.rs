use std::time::{Duration, SystemTime};

use crate::util::tpm::TpmSigner;
use amos_common::device_jwt::MAX_TOKEN_LIFETIME;

use base64::Engine;
use tracing::debug;

const REFRESH_BEFORE: Duration = Duration::from_secs(30);

pub struct DeviceJwtProvider {
    current_token: String,
    expire_time: SystemTime,
    signer: TpmSigner,
}

impl DeviceJwtProvider {
    pub fn new(signer: TpmSigner) -> Self {
        Self {
            current_token: String::new(),
            expire_time: std::time::UNIX_EPOCH,
            signer,
        }
    }

    pub fn token(&mut self, device_uuid: &str) -> anyhow::Result<&str> {
        if SystemTime::now() + REFRESH_BEFORE > self.expire_time {
            debug!("Refreshing device JWT token");

            self.expire_time = SystemTime::now() + Duration::from_secs(MAX_TOKEN_LIFETIME as u64);
            self.current_token = self.create_signed_jwt(device_uuid, self.expire_time)?;
        }

        Ok(&self.current_token)
    }

    fn create_signed_jwt(
        &mut self,
        device_uuid: &str,
        expire_time: SystemTime,
    ) -> anyhow::Result<String> {
        let header = jsonwebtoken::Header {
            alg: jsonwebtoken::Algorithm::RS256,
            ..Default::default()
        };

        let claims = amos_common::device_jwt::Claims {
            sub: device_uuid.to_owned(),
            exp: expire_time.duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64,
            role: "device".to_owned(),
        };

        let header_json = serde_json::to_string(&header).unwrap();
        let claims_json = serde_json::to_string(&claims).unwrap();

        let header_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(header_json);
        let claims_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims_json);

        let data = format!("{}.{}", header_b64, claims_b64);

        let signature = self.signer.sign_data(&data)?;
        let signature_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signature);

        Ok(format!("{}.{}", data, signature_b64))
    }
}
