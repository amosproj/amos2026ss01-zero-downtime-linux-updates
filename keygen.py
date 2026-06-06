import base64
from cryptography.hazmat.primitives.asymmetric import ed25519
from cryptography.hazmat.primitives.serialization import Encoding, PrivateFormat, PublicFormat, NoEncryption
import json
from pathlib import Path

# generate
sk = ed25519.Ed25519PrivateKey.generate()
pk = sk.public_key()

# PEM files (PKCS#8 private, SubjectPublicKeyInfo public)
priv_pem = sk.private_bytes(encoding=Encoding.PEM, format=PrivateFormat.PKCS8, encryption_algorithm=NoEncryption())
pub_pem = pk.public_bytes(encoding=Encoding.PEM, format=PublicFormat.SubjectPublicKeyInfo)

Path('ed25519_pkcs8.pem').write_bytes(priv_pem)
Path('ed25519_pub.pem').write_bytes(pub_pem)

# Raw bytes for JWK
sk_raw = sk.private_bytes(encoding=Encoding.Raw, format=PrivateFormat.Raw, encryption_algorithm=NoEncryption())
pk_raw = pk.public_bytes(encoding=Encoding.Raw, format=PublicFormat.Raw)
b64u = lambda b: base64.urlsafe_b64encode(b).rstrip(b'=').decode()

jwk_priv = {"kty":"OKP","crv":"Ed25519","d":b64u(sk_raw),"x":b64u(pk_raw)}
jwk_pub  = {"kty":"OKP","crv":"Ed25519","x":b64u(pk_raw)}

Path('ed25519_jwk_private.json').write_text(json.dumps(jwk_priv))
Path('ed25519_jwk_public.json').write_text(json.dumps(jwk_pub))

print('Wrote: ed25519_pkcs8.pem, ed25519_pub.pem, ed25519_jwk_private.json, ed25519_jwk_public.json')
