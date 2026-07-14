## General TPM Lifecycle Flow

The TPM acts as the cryptographic root of trust for device onboarding and ongoing runtime API authentication. The architecture follows a four-stage lifecycle:

### 1. Hardware Provisioning (Endorsement Key)
Every target device (or emulated vTPM) is initialized with a unique, permanent **Endorsement Key (EK)** factory-burned or generated into the TPM ecosystem. This key resides securely inside the TPM and its public portion is accessible via the persistent handle `0x81010001`.

### 2. Administrative Pre-Authorization
Before a physical device is allowed to connect to the cloud, an administrator or onboarding script extracts the device's unique Serial Number and its TPM Endorsement Public Key. This identity pairing is submitted to the backend via the administrative `POST /v1/pending-device-registrations` endpoint, placing the device into a "trusted pending" state.

### 3. Device Self-Registration
Upon its first boot, the edge device orchestrator generates a new, localized **Operational Signing Key Pair** inside its own TPM. The device then hits the public `/register` endpoint, presenting its hardware identity (EK) along with its new Operational Public Key. The server verifies the EK against the pending registrations database and saves the device's Operational Public Key.

### 4. Runtime JWT Authentication
For all subsequent operational API interactions (e.g., polling for updates, streaming logs):
* **Device-Side:** The orchestrator generates a JWT containing a `"role": "device"` claim and requests the local TPM to sign the token using its internal Operational Signing Key.
* **Server-Side:** The backend auth middleware intercepts the request, detects the device role, retrieves the matching public key from the database, and cryptographically verifies the signature via RS256.

## tpm2-tools reference for TPM debugging

For debugging the vTPM inside the edge ipc VM (or the real hardware), here are some commands for working with the TPM.

The *tpm2tools* package must be installed to have the commands below available.

Checking for TPM availability / functionality:

```bash
# List the tpm device in the dev shadow fs - should yield "/dev/tpm0 /dev/tpmrm0"
ls /dev/tpm*

# Read the TPM version of the tpm device above - should yield "2", (indicating a TPM 2.0)
cat /sys/class/tpm/tpm0/tpm_version_major

# List all handles to keys stored inside the TPM - should not error
# and return nothing (uninitialized) or the handle to a saved key
sudo tpm2_getcap handles-persistent
```

Initialize the TPM and create a persistent signing key:

```bash
# Initialize the owner context
sudo tpm2_createprimary -C o -c primary.ctx

# Create a keypair
sudo tpm2_create -C primary.ctx -G rsa -u key.pub -r key.priv # key.priv is an encrypted blow, not a readable private key
# Possibly add the following flag to not require an auth session when signing:
#   -a "sign|fixedtpm|fixedparent|sensitivedataorigin|userwithauth"
# Load the blob into the TPM
sudo tpm2_load -C primary.ctx -u key.pub -r key.priv -c key.ctx
# Read and export the public key (ascii armored)
sudo tpm2_readpublic -c key.ctx -f pem -o pubkey.pem
# Verify the public key can be read by openssl
sudo openssl rsa -pubin -in pubkey.pem -text -noout

# Persist the key into the owner context at the given handle
sudo tpm2_evictcontrol -C o -c key.ctx 0x81000000
# List the persistent handles (should now contain the handle of the line above)
sudo tpm2_getcap handles-persistent
```

Sign a test file inside the TPM and verify the signature against the known public key:

```bash
date > data.txt
sudo tpm2_sign -c 0x81000000 -g sha256 -f plain -o sig.bin data.txt
sudo openssl dgst -sha256 -verify pubkey.pem -signature sig.bin data.txt
```
