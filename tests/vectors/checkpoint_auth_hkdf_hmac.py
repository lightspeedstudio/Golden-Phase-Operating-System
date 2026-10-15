"""Independent HKDF-SHA256 + HMAC-SHA256 for GPC1 checkpoint vectors.

TEST-ONLY. Not a production secret. Mirrors kernel::crypto::keyring::hkdf_sha256
(Extract then one-block Expand). Does not import seal_record or derive_k_checkpoint.
"""

from __future__ import annotations

import hashlib
import hmac
import sys


def hkdf_sha256(ikm: bytes, salt: bytes, info: bytes) -> bytes:
    prk = hmac.new(salt, ikm, hashlib.sha256).digest()
    return hmac.new(prk, info + b"\x01", hashlib.sha256).digest()


def lp(data: bytes) -> bytes:
    return len(data).to_bytes(4, "little") + data


# TEST-ONLY disposable development secret. Not a production key.
SECRET = bytes([0x11] * 32)
DEVICE_ID = bytes([0xA1] * 32)
BOOT_SESSION = bytes([0xB0] * 32)
KEY_EPOCH = 7
BACKEND = b"EPHEMERAL_PROVISIONED_SECRET"
SALT = b"GPOS/checkpoint/v1"


def k_checkpoint() -> bytes:
    info = (
        lp(DEVICE_ID)
        + lp(BOOT_SESSION)
        + lp(KEY_EPOCH.to_bytes(4, "little"))
        + lp(BACKEND)
    )
    return hkdf_sha256(SECRET, SALT, info)


def main() -> int:
    k = k_checkpoint()
    sys.stdout.write(k.hex() + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
