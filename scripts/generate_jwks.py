# This script generates a keypair for signing/verifying JWTs during testing.
# Apparently, there is not a single generator on the internet for elliptic JWKs...

import argparse
from cryptography.hazmat.primitives.asymmetric import ed25519, rsa
from cryptography.hazmat.primitives.serialization import Encoding, NoEncryption, PrivateFormat, PublicFormat
import sys


def generate_ed25519_pair() -> tuple[bytes, bytes]:
    keypair = ed25519.Ed25519PrivateKey.generate()

    priv_pem = keypair.private_bytes(encoding=Encoding.PEM, format=PrivateFormat.PKCS8, encryption_algorithm=NoEncryption())
    pub_pem = keypair.public_key().public_bytes(encoding=Encoding.PEM, format=PublicFormat.SubjectPublicKeyInfo)

    return (priv_pem, pub_pem)


def generate_rsa4096_pair(key_size: int = 4096) -> tuple[bytes, bytes]:
    keypair = rsa.generate_private_key(public_exponent=65537, key_size=key_size)

    priv_pem = keypair.private_bytes(encoding=Encoding.PEM, format=PrivateFormat.PKCS8, encryption_algorithm=NoEncryption())
    pub_pem = keypair.public_key().public_bytes(encoding=Encoding.PEM, format=PublicFormat.SubjectPublicKeyInfo)

    return (priv_pem, pub_pem)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Generate an ed25519 or rsa-4096 key pair for signing JWTs.")
    parser.add_argument("type",
        choices=("ed25519", "rsa-4096"),
        help="Keypair type to generate"
    )
    args = parser.parse_args()

    match args.type:
        case "ed25519":
            priv, pub = generate_ed25519_pair()
        case "rsa-4096":
            priv, pub = generate_rsa4096_pair()
        case _alg:
            print(f"Error: Unknown algorithm {_alg}", file=sys.stderr)
            sys.exit(2)

    print(priv.decode("utf-8"), pub.decode("utf-8"), sep="\n")
