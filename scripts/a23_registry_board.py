#!/usr/bin/env python3
"""A23 board classifier: AppRegistry rebuilt from recovered signed GPA.

Does not open COM. Does not modify kernel output.
Host fixtures only. Existing transcripts are not rewritten.
Empty AppRegistry after recover is a failure.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
_JOURNAL = _SCRIPTS.parent / "fpga" / "zynq" / "research" / "phasefs-persistent-journal-v1"
sys.path.insert(0, str(_JOURNAL))

from a01_raw_uart import (  # noqa: E402
    board_check_durable_in_raw,
    boot_session_roots,
    is_raw_uart_evidence,
)

INSTALL_RE = re.compile(r"installed app=(\S+) root=([0-9a-f]{64})")
ROOT_RE = re.compile(r"AppRegistryRoot: ([0-9a-f]{64}) generation=(\d+)")
ROW_RE = re.compile(r"^([0-9a-f]{64}) (\S+) (\S+) gen=(\d+) seq=(\d+)\s*$", re.M)
LIVE_RE = re.compile(r"blocks_live:\s+(\d+)")
COMMITTED_RE = re.compile(r"committed:\s+(\d+)")
RECON_RE = re.compile(r"reconstructed recovered=(\d+) skipped=(\d+)")

STORE_FILES = (
    "package.gpa",
    "install.meta",
    "manifest.phase",
    "hooks.phase",
    "code.phase",
    "sentinel.psm1",
)
A12_PATH = "/a12.bin"
A12_PAYLOAD = "a12-extent"
EMPTY_PREFIX = "975cfcd8"

EXIT_ACCEPT = 0
EXIT_REJECT = 1


def _forbidden(text: str) -> str | None:
    if "journal i/o" in text:
        return "JOURNAL_IO"
    if re.search(r"^result=PASS\s*$", text, flags=re.M):
        return "HOST_RESULT_LINE"
    if re.search(r"^===== .+ (PASS|FAIL)", text, flags=re.M):
        return "HOST_STEP_BANNER"
    return None


def _checksum_line(text: str, path: str) -> str | None:
    pat = re.compile(
        r"checksum\s+" + re.escape(path) + r"\s*\r?\n(\[[0-9a-fA-Fx, ]+\])",
        re.M,
    )
    m = pat.search(text)
    return m.group(1) if m else None


def _live_blocks(text: str) -> list[int]:
    return [int(m.group(1)) for m in LIVE_RE.finditer(text)]


def _txlog_committed(text: str) -> list[int]:
    return [int(m.group(1)) for m in COMMITTED_RE.finditer(text)]


def _registry_roots(text: str) -> list[tuple[str, int]]:
    return [(m.group(1), int(m.group(2))) for m in ROOT_RE.finditer(text)]


def _rows_for_app(text: str, app: str) -> list[re.Match[str]]:
    return [m for m in ROW_RE.finditer(text) if m.group(2) == app]


def _list_block_counts(text: str, app: str) -> list[int]:
    matches = list(ROOT_RE.finditer(text))
    counts: list[int] = []
    for i, m in enumerate(matches):
        start = m.end()
        end = matches[i + 1].start() if i + 1 < len(matches) else len(text)
        counts.append(len(_rows_for_app(text[start:end], app)))
    return counts


def classify_install(text: str) -> tuple[int, str, dict]:
    raw_ok, raw_reason = is_raw_uart_evidence(text)
    if not raw_ok:
        return EXIT_REJECT, raw_reason, {}
    bad = _forbidden(text)
    if bad:
        return EXIT_REJECT, bad, {}
    roots = boot_session_roots(text)
    if len(roots) != 1:
        return EXIT_REJECT, "INSTALL_BOOT_EPOCH", {"roots": roots}
    if "RAM PhaseFs only" in text or "no partition 1" in text:
        return EXIT_REJECT, "RAM_FALLBACK_BOOT", {}
    if "backend: SD_PARTITION_1" not in text or "durable: YES" not in text:
        return EXIT_REJECT, "WAL_NOT_DURABLE", {}
    check_ok, check_reason = board_check_durable_in_raw(text)
    if not check_ok:
        return EXIT_REJECT, check_reason, {}
    inst = INSTALL_RE.search(text)
    if not inst:
        return EXIT_REJECT, "NO_INSTALL", {}
    app = inst.group(1)
    pkg_root = inst.group(2)
    store = f"/phase/packages/{pkg_root}"
    for name in STORE_FILES:
        if name not in text:
            return EXIT_REJECT, f"MISSING_{name}", {"root": pkg_root}
        if _checksum_line(text, f"{store}/{name}") is None:
            return EXIT_REJECT, f"NO_CHECKSUM_{name}", {"root": pkg_root}
    committed = _txlog_committed(text)
    if len(committed) < 2:
        return EXIT_REJECT, "NO_TXLOG_PAIR", {}
    if committed[0] != committed[-1]:
        return EXIT_REJECT, "TXLOG_COMMITTED_CHANGED", {"committed": committed}
    live = _live_blocks(text)
    if len(live) < 2:
        return EXIT_REJECT, "NO_EXTMAP_PAIR", {}
    if live[-1] < 1:
        return EXIT_REJECT, "EXTENTS_EMPTY", {"live": live}
    if A12_PAYLOAD not in text:
        return EXIT_REJECT, "A12_PAYLOAD_MISSING", {}
    if "SENTINEL_APP_OUTPUT:" not in text:
        return EXIT_REJECT, "NO_LAUNCH", {}
    reg = _registry_roots(text)
    if not reg:
        return EXIT_REJECT, "NO_APPREGISTRY_ROOT", {}
    last_root, last_gen = reg[-1]
    if last_root.startswith(EMPTY_PREFIX) or last_gen == 0:
        return EXIT_REJECT, "EMPTY_REGISTRY_AFTER_INSTALL", {
            "root": last_root,
            "generation": last_gen,
        }
    rows = _rows_for_app(text, app)
    if len(rows) < 1:
        return EXIT_REJECT, "APP_NOT_LISTED", {"app": app}
    if any(c != 1 for c in _list_block_counts(text, app) if c > 0):
        return EXIT_REJECT, "DUPLICATE_REGISTRY_ENTRY", {"app": app}
    info = {
        "root": pkg_root,
        "app": app,
        "run_id": roots[0],
        "committed": committed[0],
        "blocks_live": live[-1],
        "payload": A12_PAYLOAD,
        "registry_root": last_root,
        "generation": last_gen,
        "checksums": {
            name: _checksum_line(text, f"{store}/{name}") for name in STORE_FILES
        },
    }
    return EXIT_ACCEPT, "INSTALL_A23", info


def classify_recover(text: str, install_info: dict) -> tuple[int, str]:
    raw_ok, raw_reason = is_raw_uart_evidence(text)
    if not raw_ok:
        return EXIT_REJECT, raw_reason
    bad = _forbidden(text)
    if bad:
        return EXIT_REJECT, bad
    roots = boot_session_roots(text)
    if len(roots) != 1:
        return EXIT_REJECT, "RECOVER_BOOT_EPOCH"
    if roots[0] == install_info["run_id"]:
        return EXIT_REJECT, "RECOVER_SAME_BOOT_EPOCH"
    if "RAM PhaseFs only" in text or "no partition 1" in text:
        return EXIT_REJECT, "RAM_FALLBACK_RECOVER"
    check_ok, check_reason = board_check_durable_in_raw(text)
    if not check_ok:
        return EXIT_REJECT, check_reason
    if INSTALL_RE.search(text):
        return EXIT_REJECT, "REINSTALL_ON_RECOVER"
    pkg_root = install_info["root"]
    store = f"/phase/packages/{pkg_root}"
    if store not in text:
        return EXIT_REJECT, "STORE_MISSING_AFTER_RESET"
    for name, expected in install_info["checksums"].items():
        got = _checksum_line(text, f"{store}/{name}")
        if got is None:
            return EXIT_REJECT, f"NO_CHECKSUM_AFTER_RESET_{name}"
        if got != expected:
            return EXIT_REJECT, f"HASH_MISMATCH_{name}"
    live = _live_blocks(text)
    if not live or live[-1] < 1:
        return EXIT_REJECT, "EXTENTS_EMPTY_AFTER_RESET"
    if live[-1] < install_info["blocks_live"]:
        return EXIT_REJECT, "EXTENTS_SHRANK_AFTER_RESET"
    if install_info["payload"] not in text:
        return EXIT_REJECT, "PAYLOAD_MISSING_AFTER_RESET"
    reg = _registry_roots(text)
    if not reg:
        return EXIT_REJECT, "NO_APPREGISTRY_ROOT_AFTER_RESET"
    if any(root.startswith(EMPTY_PREFIX) or gen == 0 for root, gen in reg):
        return EXIT_REJECT, "EMPTY_REGISTRY_AFTER_RESET"
    app = install_info["app"]
    counts = _list_block_counts(text, app)
    if not any(c == 1 for c in counts):
        return EXIT_REJECT, "APP_NOT_LISTED_AFTER_RESET"
    if any(c > 1 for c in counts):
        return EXIT_REJECT, "DUPLICATE_REGISTRY_ENTRY"
    first_root, first_gen = reg[0]
    if any(root != first_root or gen != first_gen for root, gen in reg):
        return EXIT_REJECT, "REGISTRY_NOT_IDEMPOTENT"
    if "SENTINEL_APP_OUTPUT:" not in text:
        return EXIT_REJECT, "NO_LAUNCH_AFTER_RESET"
    recon = RECON_RE.search(text)
    if not recon:
        return EXIT_REJECT, "NO_RECONSTRUCT"
    if int(recon.group(1)) < 1:
        return EXIT_REJECT, "RECONSTRUCT_RECOVERED_ZERO"
    return EXIT_ACCEPT, "RECOVER_A23"


def _selftest() -> None:
    root = "ab" * 32
    app = "hello-a7f3c91e"
    reg = "11" * 32
    install = (
        "  [ SEALED] BootSessionRoot:     aa11bb22\n"
        "gpos> fsj status\n"
        "backend: SD_PARTITION_1\n"
        "durable: YES\n"
        "gpos> fsj check\n"
        "fsj check: PASS\n"
        "DURABLE: YES\n"
        "gpos> txlog\n"
        "committed: 0\n"
        "gpos> app list\n"
        "AppRegistryRoot: "
        + EMPTY_PREFIX
        + ("00" * 28)
        + " generation=0\n"
        "gpos> extmap\n"
        "blocks_live: 4\n"
        "gpos> app install hello.gpa\n"
        f"installed app={app} root={root}\n"
        f"gpos> ls /phase/packages/{root}\n"
        "code.phase\nhooks.phase\ninstall.meta\nmanifest.phase\npackage.gpa\nsentinel.psm1\n"
    )
    for name, digest in [
        ("package.gpa", "[1]"),
        ("install.meta", "[2]"),
        ("manifest.phase", "[3]"),
        ("hooks.phase", "[4]"),
        ("code.phase", "[5]"),
        ("sentinel.psm1", "[6]"),
    ]:
        install += f"gpos> checksum /phase/packages/{root}/{name}\n{digest}\n"
    install += (
        "gpos> txlog\n"
        "committed: 0\n"
        "gpos> app list\n"
        f"AppRegistryRoot: {reg} generation=1\n"
        f"{'cd' * 32} {app} Installed gen=1 seq=1\n"
        "gpos> app run hello-a7f3c91e\n"
        "SENTINEL_APP_OUTPUT:\n"
        f"package={app}\n"
        "gpos> write /a12.bin "
        + A12_PAYLOAD
        + "\n"
        "gpos> extmap\n"
        "blocks_live: 5\n"
        f"gpos> cat {A12_PATH}\n"
        + A12_PAYLOAD
        + "\n"
        "gpos>\n"
    )
    code, reason, info = classify_install(install)
    assert code == EXIT_ACCEPT, (code, reason, info)
    overwrite = install.replace("blocks_live: 5", "blocks_live: 4")
    code, reason, info2 = classify_install(overwrite)
    assert code == EXIT_ACCEPT, (code, reason, info2)
    recover = (
        "  [ SEALED] BootSessionRoot:     cc33dd44\n"
        f"gpos> ls /phase/packages/{root}\n"
        "code.phase\nhooks.phase\ninstall.meta\nmanifest.phase\npackage.gpa\nsentinel.psm1\n"
    )
    for name, digest in [
        ("package.gpa", "[1]"),
        ("install.meta", "[2]"),
        ("manifest.phase", "[3]"),
        ("hooks.phase", "[4]"),
        ("code.phase", "[5]"),
        ("sentinel.psm1", "[6]"),
    ]:
        recover += f"gpos> checksum /phase/packages/{root}/{name}\n{digest}\n"
    recover += (
        f"gpos> cat {A12_PATH}\n"
        + A12_PAYLOAD
        + "\n"
        "gpos> extmap\n"
        "blocks_live: 5\n"
        "gpos> app list\n"
        f"AppRegistryRoot: {reg} generation=1\n"
        f"{'cd' * 32} {app} Installed gen=1 seq=1\n"
        "gpos> app reconstruct\n"
        "reconstructed recovered=1 skipped=0\n"
        f"AppRegistryRoot: {reg} generation=1\n"
        "gpos> app list\n"
        f"AppRegistryRoot: {reg} generation=1\n"
        f"{'cd' * 32} {app} Installed gen=1 seq=1\n"
        "gpos> app run hello-a7f3c91e\n"
        "SENTINEL_APP_OUTPUT:\n"
        f"package={app}\n"
        "gpos> fsj check\n"
        "fsj check: PASS\n"
        "DURABLE: YES\n"
        "gpos>\n"
    )
    code, reason = classify_recover(recover, info)
    assert code == EXIT_ACCEPT, (code, reason)
    empty = recover.replace(f"AppRegistryRoot: {reg} generation=1", f"AppRegistryRoot: {EMPTY_PREFIX}{('00' * 28)} generation=0")
    empty = empty.replace(f"{'cd' * 32} {app} Installed gen=1 seq=1\n", "")
    code, reason = classify_recover(empty, info)
    assert code == EXIT_REJECT and reason == "EMPTY_REGISTRY_AFTER_RESET", (code, reason)
    print("a23_registry_board selftest: ok")


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[1] == "--selftest":
        _selftest()
        return 0
    print("usage: a23_registry_board.py --selftest", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
