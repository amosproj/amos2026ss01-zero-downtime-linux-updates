# tpm2-tools reference for TPM debugging

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
