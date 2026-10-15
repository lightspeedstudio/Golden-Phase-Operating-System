# Copyright LightSpeed Studio LLC
# SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
"""System 6A host-only verifier.

For System 6A, use an external host-only crypto verifier such as OpenSSL or
Python Cryptography to decrypt and verify the fixtures. Do not add owned
AES/HMAC implementation code yet.

AES-256 via Python Cryptography (OpenSSL backend). HMAC-SHA256 via hashlib.
Does not implement AES or HMAC in the owned crate. Does not print key, IV,
HMAC, or recovered payload bytes. This is not the FSBL path: installed
zynq_fsbl decrypts through DevCfg PCAP into the PL AES/HMAC unit.

Exit 0: KAT recovered byte-for-byte and HMAC verified.
Exit 2: framing parsed; inner AES/HMAC still unresolved (or oracle missing).
"""
from __future__ import annotations

import hashlib
import hmac
import struct
import sys
from pathlib import Path

try:
    from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
except ImportError:
    print("SYS06A_VERIFY skip (cryptography missing)")
    sys.exit(2)

OWNED = Path(__file__).resolve().parents[1]
ORACLE = OWNED / "build" / "oracle"
LOCKED = ORACLE / "BOOT_SYS06_ORACLE.BIN"
NKY = ORACLE / "sys06_dev.nky"
KAT = ORACLE / "ddr_kat.bin"
SYS06A = ORACLE / "sys06a"

STREAM_HEADER = 204
NOOP_TRAILER = 64
IV_OFF = 116


def u32(b: bytes, o: int) -> int:
    return struct.unpack_from("<I", b, o)[0]


def hx(s: str) -> bytes:
    return bytes.fromhex("".join(c for c in s if c in "0123456789abcdefABCDEF"))


def parse_nky(text: str) -> dict[str, bytes]:
    out: dict[str, bytes] = {}
    for raw in text.splitlines():
        p = raw.split()
        if len(p) >= 3 and p[0] == "Key" and p[1] == "0":
            out["Key0"] = hx(p[2])
        elif len(p) >= 3 and p[0] == "Key" and p[1] == "StartCBC":
            out["IV"] = hx(p[2])
        elif len(p) >= 3 and p[0] == "Key" and p[1] == "HMAC":
            out["HMAC"] = hx("".join(p[2:]))
    return out


def p1_blob(boot: bytes) -> bytes:
    ph = u32(boot, 0x9C)
    n = u32(boot, u32(boot, 0x98) + 4)
    for i in range(n):
        o = ph + i * 0x40
        if u32(boot, o + 0xC) == 0x03000000:
            enc = u32(boot, o)
            off = u32(boot, o + 0x14) * 4
            return boot[off : off + enc * 4]
    raise SystemExit("no KAT partition")


def word_swap(b: bytes) -> bytes:
    o = bytearray(b)
    for i in range(0, len(o) - 3, 4):
        o[i : i + 4] = o[i : i + 4][::-1]
    return bytes(o)


def bitrev32(x: int) -> int:
    x &= 0xFFFFFFFF
    x = ((x & 0x55555555) << 1) | ((x >> 1) & 0x55555555)
    x = ((x & 0x33333333) << 2) | ((x >> 2) & 0x33333333)
    x = ((x & 0x0F0F0F0F) << 4) | ((x >> 4) & 0x0F0F0F0F)
    x = ((x & 0x00FF00FF) << 8) | ((x >> 8) & 0x00FF00FF)
    return ((x << 16) | (x >> 16)) & 0xFFFFFFFF


def bitrev8(x: int) -> int:
    return int("{:08b}".format(x)[::-1], 2)


def zynq_word_swap(b: bytes) -> bytes:
    """7-series 32-bit byte-swap plus per-byte bit-reverse (host mapping only)."""
    o = bytearray(b)
    for i in range(0, len(o) - 3, 4):
        w = o[i : i + 4][::-1]
        o[i : i + 4] = bytes(bitrev8(x) for x in w)
    return bytes(o)


def nist_cbc(key: bytes, iv: bytes, ct: bytes) -> bytes:
    if len(ct) % 16:
        ct = ct[: len(ct) - (len(ct) % 16)]
    d = Cipher(algorithms.AES(key), modes.CBC(iv)).decryptor()
    return d.update(ct) + d.finalize()


def nist_ecb_block(key: bytes, block: bytes, decrypt: bool) -> bytes:
    c = Cipher(algorithms.AES(key), modes.ECB())
    op = c.decryptor() if decrypt else c.encryptor()
    return op.update(block) + op.finalize()


def fpga_shaped_cbc(key: bytes, iv: bytes, ct: bytes) -> bytes:
    """NIST AES-ECB (Cryptography/OpenSSL) with 32-bit bit-reverse wrapping.

    This is not an owned AES implementation. It is a host mapping around the
    same engine used by `openssl enc -aes-256-ecb`.
    """
    if len(ct) % 16:
        ct = ct[: len(ct) - (len(ct) % 16)]
    out = bytearray(len(ct))
    prev = None
    ivw = [int.from_bytes(iv[i : i + 4], "big") for i in range(0, 16, 4)]
    for blk in range(0, len(ct), 16):
        c_le = [struct.unpack_from("<I", ct, blk + 4 * i)[0] for i in range(4)]
        st = [bitrev32(w) for w in c_le]
        nist = b"".join(w.to_bytes(4, "big") for w in st)
        st = [int.from_bytes(nist_ecb_block(key, nist, True)[i : i + 4], "big") for i in range(0, 16, 4)]
        st = [bitrev32(w) for w in st]
        if prev is None:
            pw = [st[i] ^ ivw[i] for i in range(4)]
        else:
            pw = [st[i] ^ prev[i] for i in range(4)]
        prev = [bitrev32(w) for w in c_le]
        for i in range(4):
            struct.pack_into("<I", out, blk + 4 * i, pw[i])
    return bytes(out)


def find_kat(pt: bytes, kat: bytes) -> int:
    idx = pt.find(kat)
    if idx >= 0:
        return idx
    return word_swap(pt).find(kat)


def recover(key: bytes, iv_nky: bytes, hdr_iv: bytes, ct: bytes, kat: bytes) -> int:
    ivs = (iv_nky, hdr_iv, word_swap(hdr_iv))
    keys = (key, word_swap(key), zynq_word_swap(key))
    for k in keys:
        for iv in ivs:
            idx = find_kat(nist_cbc(k, iv, ct), kat)
            if idx >= 0:
                return idx
            idx = find_kat(fpga_shaped_cbc(k, iv, ct), kat)
            if idx >= 0:
                return idx
            idx = find_kat(nist_cbc(k, zynq_word_swap(iv), zynq_word_swap(ct)), kat)
            if idx >= 0:
                return idx
            idx = find_kat(zynq_word_swap(nist_cbc(k, zynq_word_swap(iv), zynq_word_swap(ct))), kat)
            if idx >= 0:
                return idx
    return -1


def hmac_ok(mac_key: bytes, msg: bytes, tag: bytes) -> bool:
    if len(tag) != 32 or len(mac_key) != 32:
        return False
    got = hmac.new(mac_key, msg, hashlib.sha256).digest()
    return hmac.compare_digest(got, tag)


def verify_blob(keys: dict[str, bytes], blob: bytes, kat: bytes) -> int:
    if len(blob) < STREAM_HEADER + NOOP_TRAILER + 16:
        return -1
    hdr_iv = blob[IV_OFF : IV_OFF + 16]
    ct = blob[STREAM_HEADER : len(blob) - NOOP_TRAILER]
    return recover(keys["Key0"], keys["IV"], hdr_iv, ct, kat)


def main() -> int:
    if not LOCKED.exists() or not NKY.exists() or not KAT.exists():
        print("SYS06A_VERIFY skip (oracle or nky missing)")
        return 2
    keys = parse_nky(NKY.read_text(encoding="ascii"))
    if len(keys.get("Key0", b"")) != 32 or len(keys.get("IV", b"")) != 16:
        print("SYS06A_VERIFY nky_field_len_bad")
        return 2
    try:
        from cryptography.hazmat.backends.openssl.backend import backend

        print("aes_engine cryptography/openssl", backend.openssl_version_text())
    except Exception:
        print("aes_engine cryptography")
    kat = KAT.read_bytes()
    blob = p1_blob(LOCKED.read_bytes())
    hdr_iv = blob[IV_OFF : IV_OFF + 16]
    ct = blob[STREAM_HEADER : len(blob) - NOOP_TRAILER]
    print("framing header=204 cipher=%d trailer=64" % len(ct))
    print("iv_hdr_eq_wordswap_nky", hdr_iv == word_swap(keys["IV"]))
    idx = verify_blob(keys, blob, kat)
    print("KAT_RECOVER", "yes" if idx >= 0 else "no")
    mutated = bytearray(blob)
    mutated[STREAM_HEADER] ^= 1
    idx_m = verify_blob(keys, bytes(mutated), kat)
    if idx < 0:
        print("KAT_MUTATION_STILL_ABSENT")
    elif idx_m < 0:
        print("KAT_MUTATION_REJECTED")
    else:
        print("KAT_MUTATION_STILL_MATCHES")
    mini_ok = True
    if SYS06A.exists():
        for n in (15, 16, 17, 31, 32, 33, 255, 256, 257):
            boot_p = SYS06A / ("BOOT_KAT_%d.BIN" % n)
            kat_p = SYS06A / ("kat_%d.bin" % n)
            if not boot_p.exists() or not kat_p.exists():
                continue
            if verify_blob(keys, p1_blob(boot_p.read_bytes()), kat_p.read_bytes()) < 0:
                mini_ok = False
        print("MINI_KAT_RECOVER", "yes" if mini_ok and idx >= 0 else "no")
    hmac_pass = False
    if idx >= 0:
        pt = nist_cbc(keys["Key0"], keys["IV"], ct)
        if kat not in pt:
            pt = fpga_shaped_cbc(keys["Key0"], keys["IV"], ct)
        # Tag is inside the decrypted stream. Try the last 32 bytes of plaintext.
        tag = pt[-32:]
        body = pt[:-32]
        hmac_pass = hmac_ok(keys.get("HMAC", b""), body, tag)
        print("HMAC_VERIFY", "yes" if hmac_pass else "no")
    else:
        print("HMAC_VERIFY skipped")
    if idx >= 0 and hmac_pass and idx_m < 0:
        return 0
    print("SYS06A_FRAMING_UNRESOLVED")
    return 2


if __name__ == "__main__":
    sys.exit(main())
