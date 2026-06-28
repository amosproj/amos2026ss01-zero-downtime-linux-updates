use std::fs;
use std::str::FromStr as _;

use anyhow::Result;
use rsa::pkcs8::EncodePublicKey as _;
use rsa::{BigUint, RsaPublicKey};
use tracing::{debug, info, warn};
use tss_esapi::handles::{KeyHandle, PersistentTpmHandle};
use tss_esapi::interface_types::algorithm::HashingAlgorithm;
use tss_esapi::interface_types::resource_handles::Hierarchy;
use tss_esapi::structures::{HashScheme, MaxBuffer, Public, SignatureScheme};
use tss_esapi::{Context, TctiNameConf, WrapperErrorKind};

// Persistent handle where the RSA endorsement key is mapped to
const RSA_EK_PERSISTENT_HANDLE: u32 = 0x8101_0001;

const PERSISTENT_SIGNING_HANDLE: u32 = 0x8100_0000;

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

        // Try loading the persistent signing key
        let persistent_handle = PersistentTpmHandle::new(PERSISTENT_SIGNING_HANDLE)?;

        let key_handle = match ctx.tr_from_tpm_public(persistent_handle.into()) {
            Ok(object_handle) => {
                // Handle exists → continue
                KeyHandle::from(object_handle)
            }

            Err(tss_esapi::Error::Tss2Error(rc)) => {
                match rc.kind() {
                    Some(tss_esapi::constants::response_code::Tss2ResponseCodeKind::Handle) => {
                        info!("Signing key not present, starting initialization routine");

                        create_signing_key(&mut ctx)?
                    }

                    _ => {
                        return Err(tss_esapi::Error::Tss2Error(rc).into());
                    }
                }
            }

            Err(e) => {
                return Err(e.into());
            }
        };

        // Read public area
        let (public, _name, _qualified_name) = ctx.read_public(key_handle)?;
        info!("Key exists, public area loaded");

        let pubkey = armor_rsa_public_key(public)?;
        if let Err(e) = fs::write("/tmp/my_tpm_pubkey2.pem", pubkey) {
            warn!("Could not write own public key to /tpm: {}", e);
        }

        let signer = TpmSigner { ctx, key_handle };
        Ok(signer)
    }

    /// NOTE: Accessing the Endorsement key via the persistent handle as seen below is non-standardized...
    // 
    // To be safe, the NV index of the RSA EK (handle 0x1c00002) should be read which then allows reading
    // the RSA EK's certificate. This would then need to be parsed and have its public key constructed
    // from the extracted parameters.
    //
    // Instead for now, we rely on the hardware to have a persistent handle mapped at the specified address
    // by convention (ensured via the reference device). From there, the public EK can be read directly.
    pub fn read_endorsement_key(&mut self) -> anyhow::Result<String> {
        let ek_handle = PersistentTpmHandle::new(RSA_EK_PERSISTENT_HANDLE)?;

        // Convert persistent -> transient ESYS handle
        let transient = self.ctx.tr_from_tpm_public(ek_handle.into())?;
        let key_handle = KeyHandle::from(transient);

        // Read public key from transient handle
        let (public, _, _) = self.ctx.read_public(key_handle)?;

        let pem = armor_rsa_public_key(public)?;

        Ok(pem)
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

        Ok(signature_bytes)
    }
}

pub fn create_signing_key(context: &mut Context) -> anyhow::Result<KeyHandle> {
    Ok(KeyHandle::Null)
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
