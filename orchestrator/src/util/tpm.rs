use std::str::FromStr as _;

use log::debug;
use rsa::pkcs8::EncodePublicKey as _;
use rsa::{BigUint, RsaPublicKey};
use tss_esapi::constants::SessionType;
use tss_esapi::{Context, TctiNameConf};
use tss_esapi::interface_types::algorithm::HashingAlgorithm;
use tss_esapi::interface_types::resource_handles::Hierarchy;
use tss_esapi::handles::{KeyHandle, PersistentTpmHandle};
use tss_esapi::structures::{HashScheme, MaxBuffer, Public, SignatureScheme};

pub fn tpm_init() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to TPM (swtpm)
    // /dev/tpmrm0 should be preferred over /dev/tpm0 as it is resource managed
    let tcti_config = TctiNameConf::from_str("device:/dev/tpmrm0")
        .or_else(|_| TctiNameConf::from_str("device:/dev/tpm0"))?;
    debug!("Using tcti: {:?}", tcti_config);
    let mut ctx = Context::new(tcti_config)?;
    debug!("A");

    // Load your persistent key (example handle)
    let persistent_handle = PersistentTpmHandle::new(0x81000000)?;
    let object_handle = ctx.tr_from_tpm_public(persistent_handle.into())?;
    let key_handle = KeyHandle::from(object_handle);
    debug!("B");

    // ------- export pubkey
    // Read public area
    let (public, _name, _qualified_name) = ctx.read_public(key_handle)?;

    // Extract RSA parameters
    let (modulus, exponent) = match public {
        Public::Rsa { unique, parameters, .. } => {
            let modulus = unique.value().to_vec();

            // TPM stores exponent as a u32, but 0 means "default = 65537"
            let exp = parameters.exponent().value();
            let exponent = if exp == 0 { 65537 } else { exp };

            (modulus, exponent)
        }
        _ => panic!("Not an RSA key"),
    };
    println!("Modulus ({} bytes): {:02x?}", modulus.len(), modulus);
    println!("exponent: {}", exponent);

    let n = BigUint::from_bytes_be(&modulus);
    let e = BigUint::from(exponent);
    let pubkey = RsaPublicKey::new(n, e).unwrap();

    let spki_pem = pubkey.to_public_key_pem(Default::default()).unwrap();
    println!("{}", spki_pem);
    // ---------


    let data = b"hello world";
    let input_data = MaxBuffer::try_from(&data[..])
        .expect("Failed to create buffer for input data.");
    debug!("C");

    let (digest, ticket) = ctx.hash(
        input_data,
        HashingAlgorithm::Sha256,
        Hierarchy::Null,
    )?;
    debug!("D");

    // TODO: Possibly avoidable by creating the key without create option `withuserauth`
    let session = ctx.start_auth_session(
        None,
        None,
        None,
        SessionType::Hmac,
        tss_esapi::structures::SymmetricDefinition::AES_128_CFB,
        HashingAlgorithm::Sha256,
    )?;
    ctx.set_sessions((session, None, None));

    // RSASSA-PKCS1-v1_5 with SHA256
    let scheme = SignatureScheme::RsaSsa {
        hash_scheme: HashScheme::new(HashingAlgorithm::Sha256)
    };

    // Sign
    let signature = ctx.sign(
        key_handle,
        digest,
        scheme,
        ticket,
    )?;
    debug!("E");

    // Extract raw signature bytes
    let sig_bytes = match signature {
        tss_esapi::structures::Signature::RsaSsa(sig) => sig.signature().to_vec(),
        _ => panic!("Unexpected signature type"),
    };

    println!("Signature ({} bytes): {:02x?}", sig_bytes.len(), sig_bytes);

    Ok(())
}
