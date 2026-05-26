use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::path::Path;
use tokio::fs; // alternatively std::fs

pub async fn verify_signature(
    file_path: &Path,
    signature_bytes: &[u8],
    public_key_bytes: &[u8],
) -> bool {
    let Ok(file_content) = fs::read(file_path).await else {
        return false;
    };

    let Ok(public_key) = VerifyingKey::try_from(public_key_bytes) else {
        return false;
    };

    let Ok(signature) = Signature::from_slice(signature_bytes) else {
        return false;
    };

    public_key.verify(&file_content, &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    async fn setup_test_file(name: &str, content: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(name);
        fs::write(&path, content).await.unwrap();
        path
    }

    #[tokio::test]
    async fn test_verification_logic() {
        let mut csprng = OsRng;
        let signing_key: SigningKey = SigningKey::generate(&mut csprng);
        let public_key = signing_key.verifying_key();

        let file_content = b"Authorized Update v1.0";
        let test_file = setup_test_file("update.bin", file_content).await;
        let signature = signing_key.sign(file_content);

        // CASE 1: Valid Signature
        assert!(verify_signature(&test_file, &signature.to_bytes(), public_key.as_bytes()).await);

        // CASE 2: Invalid Signature
        let bad_sig = [0u8; 64];
        assert!(!verify_signature(&test_file, &bad_sig, public_key.as_bytes()).await);

        // CASE 3: Tampered File
        let tampered_content = b"Malicious Update v1.0";
        let tampered_file = setup_test_file("tampered_update.bin", tampered_content).await;
        assert!(
            !verify_signature(&tampered_file, &signature.to_bytes(), public_key.as_bytes()).await
        );
    }
}
