use amos_common::device_jwt::MAX_TOKEN_LIFETIME;
use crate::util::tpm::TpmSigner;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

fn prepare_jwt(device_uuid: String) -> (String, i64) {
    let header = jsonwebtoken::Header {
        alg: jsonwebtoken::Algorithm::RS256,
        ..Default::default()
    };

    let expiry = chrono::Utc::now().timestamp() + MAX_TOKEN_LIFETIME;

    let claims = amos_common::device_jwt::Claims {
        sub: device_uuid.to_owned(),
        exp: expiry,
        role: "device".to_owned(),
    };

    let header_json = serde_json::to_string(&header).unwrap();
    let claims_json = serde_json::to_string(&claims).unwrap();

    let header_b64 = URL_SAFE_NO_PAD.encode(header_json);
    let claims_b64 = URL_SAFE_NO_PAD.encode(claims_json);

    (format!("{}.{}", header_b64, claims_b64), expiry)
}

pub fn create_tpm_jwt(
    signer: &mut TpmSigner,
    device_uuid: String,
) -> Result<(String, i64), tss_esapi::Error> {
    let (header_payload, expiry) = prepare_jwt(device_uuid);

    let signature = super::tpm::sign_data(signer, header_payload.clone())?;
    let signature_b64 = URL_SAFE_NO_PAD.encode(signature);

    Ok((format!("{}.{}", header_payload, signature_b64), expiry))
}
