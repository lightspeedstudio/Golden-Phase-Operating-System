#!/usr/bin/env python3
"""Verify fault-injection UART: banner + detect latency present."""
import sys
import re


def verify(path, mode=1):
    with open(path, "r", errors="replace") as f:
        cap = f.read().replace("\r\n", "\n")
    if ("FAULT MODE %d" % mode) not in cap and ("FAULT MODE %d" % mode).replace(" ", "") not in cap.replace(" ", ""):
        # banner is "FAULT MODE n"
        if not re.search(r"FAULT MODE\s*%d" % mode, cap):
            print("MISMATCH: fault banner")
            print(cap[:200])
            return 1
    if "FAULT DONE" not in cap:
        print("MISMATCH: FAULT DONE missing")
        return 1
    m = re.search(r"FAULT DETECT latency=(\d+)", cap)
    if mode == 0:
        print("MATCH: fault mode 0 (no inject)")
        return 0
    if not m:
        print("MISMATCH: no detect latency")
        return 1
    lat = int(m.group(1))
    if "ISOLATE=footprint" not in cap:
        print("MISMATCH: missing ISOLATE observable")
        return 1
    if "PROVE=assoc_parity" not in cap:
        print("MISMATCH: missing PROVE assoc_parity observable")
        return 1
    # Real-recovery gate: a fault run MUST show EXECUTABLE restoration. The old
    # print-only "RECOVER=observed" is forbidden and fails verification, so the
    # fake recovery path cannot pass even if reintroduced.
    if "RECOVER=observed" in cap:
        print("MISMATCH: print-only 'RECOVER=observed' is forbidden (fake recovery)")
        return 1
    if "RECOVER=restored" not in cap:
        print("MISMATCH: no 'RECOVER=restored' — live recovery not executed")
        return 1
    print("MATCH: fault mode=%d detect_latency=%d recovery=restored FDIR_observables=ok" % (mode, lat))
    return 0


if __name__ == "__main__":
    path = sys.argv[1]
    mode = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    sys.exit(verify(path, mode))
