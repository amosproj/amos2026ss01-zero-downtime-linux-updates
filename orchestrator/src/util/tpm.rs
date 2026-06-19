use std::fs;
use std::str::FromStr as _;

use rsa::pkcs8::EncodePublicKey as _;
use rsa::{BigUint, RsaPublicKey};
use tracing::{debug, warn};
use tss_esapi::handles::{KeyHandle, PersistentTpmHandle};
use tss_esapi::interface_types::algorithm::HashingAlgorithm;
use tss_esapi::interface_types::resource_handles::Hierarchy;
use tss_esapi::structures::{HashScheme, MaxBuffer, Public, SignatureScheme};
use tss_esapi::{Context, TctiNameConf, WrapperErrorKind};

const PERSISTENT_HANDLE: u32 = 0x8100_0000;

pub struct TpmSigner {
    ctx: Context,
    key_handle: KeyHandle,
}

impl TpmSigner {
    pub fn new() -> anyhow::Result<Self> {
        // Connect to TPM (swtpm)
        // /dev/tpmrm0 should be preferred over /dev/tpm0 as it is resource managed
        let tcti_config = TctiNameConf::from_str("device:/dev/tpmrm0")
            .or_else(|_| TctiNameConf::from_str("device:/dev/tpm0"))?;
        debug!("Using tcti: {:?}", tcti_config);
        let mut ctx = Context::new(tcti_config)?;

        // Load your persistent key (example handle)
        let persistent_handle = PersistentTpmHandle::new(PERSISTENT_HANDLE)?;
        let object_handle = ctx.tr_from_tpm_public(persistent_handle.into())?;
        let key_handle = KeyHandle::from(object_handle);

        // Read public area
        let (public, _name, _qualified_name) = ctx.read_public(key_handle)?;
        let pubkey = armor_rsa_public_key(public)?;
        if let Err(e) = fs::write("/tmp/my_tpm_pubkey.pem", pubkey) {
            warn!("Could not write own public key to /tpm: {}", e);
        }

        let mut signer = TpmSigner { ctx, key_handle };

        let data = "hello world";
        let sig_bytes = signer.sign_data(data)?;
        println!("Signature ({} bytes): {:02x?}", sig_bytes.len(), sig_bytes);

        Ok(signer)
    }

    pub fn sign_data(&mut self, input: &str) -> anyhow::Result<Vec<u8>> {
        let input_buffer = MaxBuffer::try_from(input.as_bytes())?;

        // ensure NO session is active for TPM2_Hash
        self.ctx.clear_sessions();

        let (digest, ticket) =
            self.ctx
                .hash(input_buffer, HashingAlgorithm::Sha256, Hierarchy::Null)?;

        // RSASSA-PKCS1-v1_5 with SHA256
        let scheme = SignatureScheme::RsaSsa {
            hash_scheme: HashScheme::new(HashingAlgorithm::Sha256),
        };

        let signature = self
            .ctx
            .execute_with_nullauth_session(|ctx| -> Result<_, _> {
                ctx.sign(self.key_handle, digest, scheme, ticket)
            })?;

        let signature_bytes = match signature {
            tss_esapi::structures::Signature::RsaSsa(sig) => sig.signature().to_vec(),
            _ => anyhow::bail!("Got unexpected signature"),
        };

        // TODO: Do flush here
        // signer.ctx.flush_context(session)?;

        Ok(signature_bytes)
    }
}

fn armor_rsa_public_key(public: Public) -> Result<String, tss_esapi::Error> {
    let Public::Rsa {
        unique, parameters, ..
    } = public
    else {
        return Err(tss_esapi::Error::WrapperError(
            WrapperErrorKind::InconsistentParams,
        ));
    };

    let modulus = unique.value().to_vec();
    let n = BigUint::from_bytes_be(&modulus);

    let exponent = match parameters.exponent().value() {
        0 => 65537,
        e => e,
    };
    let e = BigUint::from(exponent);

    let pubkey = RsaPublicKey::new(n, e)
        .map_err(|_| tss_esapi::Error::WrapperError(WrapperErrorKind::InvalidParam))?;

    let pem = pubkey
        .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
        .map_err(|_| tss_esapi::Error::WrapperError(WrapperErrorKind::InvalidParam))?;

    Ok(pem)
}
