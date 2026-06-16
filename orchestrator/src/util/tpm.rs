use std::fs;
use std::str::FromStr as _;

use log::{debug, warn};
use rsa::pkcs8::EncodePublicKey as _;
use rsa::{BigUint, RsaPublicKey};
use tss_esapi::constants::SessionType;
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

pub fn tpm_init() -> Result<TpmSigner, tss_esapi::Error> {
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
    let sig_bytes = sign_data(&mut signer, data.to_string())?;
    println!("Signature ({} bytes): {:02x?}", sig_bytes.len(), sig_bytes);

    Ok(signer)
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

pub fn sign_data(signer: &mut TpmSigner, input: String) -> Result<Vec<u8>, tss_esapi::Error> {
    let input_buffer = MaxBuffer::try_from(input.as_bytes())
        .map_err(|_| tss_esapi::Error::WrapperError(WrapperErrorKind::InvalidParam))?;

    // ensure NO session is active for TPM2_Hash
    signer.ctx.clear_sessions();

    let (digest, ticket) =
        signer
            .ctx
            .hash(input_buffer, HashingAlgorithm::Sha256, Hierarchy::Null)?;

    // TODO: Possibly avoidable by creating the key without create option `withuserauth`
    let session = signer.ctx.start_auth_session(
        None,
        None,
        None,
        SessionType::Hmac,
        tss_esapi::structures::SymmetricDefinition::AES_128_CFB,
        HashingAlgorithm::Sha256,
    )?;
    signer.ctx.set_sessions((session, None, None));

    // RSASSA-PKCS1-v1_5 with SHA256
    let scheme = SignatureScheme::RsaSsa {
        hash_scheme: HashScheme::new(HashingAlgorithm::Sha256),
    };

    let signature = signer.ctx.sign(signer.key_handle, digest, scheme, ticket)?;

    let signature_bytes = match signature {
        tss_esapi::structures::Signature::RsaSsa(sig) => sig.signature().to_vec(),
        _ => {
            return Err(tss_esapi::Error::WrapperError(
                WrapperErrorKind::InconsistentParams,
            ));
        }
    };

    // TODO: Do flush here
    // signer.ctx.flush_context(session)?;

    Ok(signature_bytes)
}
