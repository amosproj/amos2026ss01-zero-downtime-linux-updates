use jsonwebtoken::{DecodingKey, Validation, decode, errors::ErrorKind};
use log::trace;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::JwtConfig;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Claims {
    pub subject: String, // subject - user ID
    pub name: String,    // user display name
    pub expiry: usize,   // expiry timestamp (Unix time)
}

// helpers that map missing/invalid -> ErrorKind::InvalidToken
pub fn get_str(claim: &Value, key: &str) -> Result<String, jsonwebtoken::errors::Error> {
    claim
        .get(key)
        .and_then(Value::as_str)
        .map(|s| s.to_owned())
        .ok_or_else(|| jsonwebtoken::errors::Error::from(ErrorKind::InvalidToken))
}

/// Validate a JWT string.
/// Returns the decoded Claims on success, or an error if the token is invalid/expired.
pub fn validate_user_token(
    token: &str,
    config: &JwtConfig,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Value>(
        token,
        &DecodingKey::from_rsa_pem(config.public_key.as_bytes())?,
        &Validation::new(jsonwebtoken::Algorithm::RS512),
    )?;
    trace!("Extracted JWT data from request: {:?}", token_data);

    let payload = token_data.claims;

    let subject = get_str(&payload, &config.subject_claim)?;
    let name = get_str(&payload, &config.name_claim)?;

    let expiry = payload
        .get("exp")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .ok_or_else(|| jsonwebtoken::errors::Error::from(ErrorKind::InvalidToken))?;

    Ok(Claims {
        subject,
        name,
        expiry,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::errors::ErrorKind;

    // Expired token -> should be rejected
    // { "sub": "1001", "name": "Joe", "exp": 1000212960 }
    #[test]
    fn rejects_expired_token() {
        let expired_jwt = "eyJhbGciOiJSUzUxMiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMDAxIiwibmFtZSI6IkpvZSBTd2Fuc29uIiwiZXhwIjoxMDAwMjEyOTYwfQ.XUI1mfbqNkBLiVvvxV9tPgbsJOkxa6xMRacnwcv8D2r855oAGBOGpLERvSelc7fdIOf822APqkgN8hIyFce1kJatLjHdiDp27XPv02dYxidE5PI5_j9oLngCTYrowlffzfbhAes6PmZHiku0PRYXcPkV3dnIhlYA89cOgDIshFNHtDCdyfL_3WQo-iaApTXvA0ZLkGPXhVAyY3IvTOHRoChrNLd6OhaWdHYukyaFZdIzvFPVmo1HGxEtCDqZ0REn66uQypz1T6iiW8ldKm5FK-rjrJMEDOJ5KjqUh84E4KK6aA4kaFwNkWM0PHGkytLDuppuB8TEoRzwPleF5h8MrLqVCPeRRigiIfxchTH38hw4DvvVTIojIuvzsApzxos6EkZ9ZGqAOMQhKTzgj4H-oZ62nTheFzjfzn6CptXXIsqAI9OS_vGcxnfvXUaI2aNLYZYvMoVfkCvc-nLLq693uqXoaV_SIQp9vTXAGXeQffBai3-FQc1_hWnUy3ezCr2H_SQnkSHmd3TonFaQADQneZf6Q5kkLQjk0Yklsv6ofFxncCabqmwnnvh6k1g0brt38SD62aTIVcnjO3-IIxa93ODRxxR8BjU9kTxhBp8hogpq10hOMp9wyfpQrJJjm_chg577P67_OpssEnvKf2KOLW2TG-f_8lkkk2Wf4fRrih4";

        let config = JwtConfig::default();
        let result = validate_user_token(expired_jwt, &config);
        assert!(result.is_err());

        match result.err().unwrap().kind() {
            ErrorKind::ExpiredSignature => (),
            other => panic!("expected ExpiredSignature, got {:?}", other),
        }
    }

    // Custom name claim key "distinguishedName" -> success
    // { "sid": "S-1-8469-2270", "distinguishedName": "Glenn Quagmire", "exp": 11860470149 }
    #[test]
    fn accepts_custom_sub_name_claim_key() {
        let jwt_with_custom_name = "eyJhbGciOiJSUzUxMiIsInR5cCI6IkpXVCJ9.eyJzaWQiOiJTLTEtODQ2OS0yMjcwIiwiZGlzdGluZ3Vpc2hlZE5hbWUiOiJHbGVubiBRdWFnbWlyZSIsImV4cCI6MTE4NjA0NzAxNDl9.JWiNoIXGNnHD6tcsohINKZN3qreU03cTjL6f_XIos6fobU-vhXCG5sbfPlJxWnnUxAizYLLJApGpyGqazwUlRhXeB0bakFxd5valy5rCfdx0dtl07NBikFnfv5Z-C4fSJTEJKth7mI_hAofL-vAhhBPpp2PxasjuBTGhINd5_EIhfmbjBVUj_IGtkAJM9NrHx_L7CKS5mWGXXspH36ZR0gbKY-mALN1_mUt07oTOuBwPxIyed6oYsAT6BEYDn9pJka4XbAqy9mwyXsW12vzVa23k81bsn1VMWvqoP605Zy0g87QBG2eY3LPcdb_Iak15qk4d0IfGkmrAPLfautVwYSKrFxZkuZrR5byoX1B6UEi01ISaMX4BhQCHsNFHDaW-Y6hBsHS4uG4un1NU4tD0VA1mwZpImaIJK2MbGLsF7e9YwxMVA1M281q6naHGrZiMdD-2I8KEPbqnl1j8-5D8wGg2dT1uhjujpHPWBiHGZ5SKi-sPdZImLTrP0WuGCSrdVIqytIkMI3r6HS8M9K4F9vHqb8AuZJGV8pHcMZOH0MF04Q4rrw2uDLQgXgtf33La1XchThg8SN7YDaEvW40DmN_ORbkkI5WsvKdMqLF35p9rbt5qA0jC98X0rFJMvdax38Rr93V-6XtKw-M2OdaCQQSsJ-FLk1bZZYaNMR3vICk";

        let mut config = JwtConfig::default();
        config.subject_claim = "sid".into();
        config.name_claim = "distinguishedName".into();

        let claims = validate_user_token(jwt_with_custom_name, &config).expect("should validate");
        assert_eq!(claims.subject, "S-1-8469-2270");
        assert_eq!(claims.name, "Glenn Quagmire");
    }

    // Wrong algorithm (token signed with RS256 but validation expects RS512) -> invalid algorithm
    // { "sub": "ff:ee:dd:cc:bb:aa", "name": "Peter Griffin", "exp": 1 }
    #[test]
    fn rejects_wrong_algorithm() {
        let jwt_rs256 = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWUsImlhdCI6MTc4MTA1MDE1MX0.dfXY8JUxnzpuCve91m6bxF0L_id5F4WfLPZXF-Vkk3CkP-A1BjmeY6kQ_ptyyUzRTbPOXgVwWVnqPkMdl0Lyr1o6Q3lmIp7cXZX77RRuLP9_m-PiZgvJdAE3jLqVhy4VYpx80o-z3RZMixAXKZfLz2bibeNaeKz0eJfbjQW1tlQgiqXFkF65qezItU2bsC0L7wztG2uWRnjv6iAR3vyGCsORutBPhjQiU1ruFlRF_kXOp8VXi7ihpOeFIgNy4wxU8vAP7SLdQMpZvC3bNMIaHGjvdyR8MMdO8idp6bpOwbf3iklWyfJGvnX1YYhvdYuijh1aHsZwcstVXDAhEzFeDw";

        let config = JwtConfig::default();
        let res = validate_user_token(jwt_rs256, &config);
        assert!(res.is_err());

        match res.err().unwrap().kind() {
            ErrorKind::InvalidAlgorithm => (),
            other => panic!("expected InvalidAlgorithm, got {:?}", other),
        }
    }
}
