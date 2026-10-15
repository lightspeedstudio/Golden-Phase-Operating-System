<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# LightSpeed Studio LLC — Stage 1 documentation index

Systems 1–12 are closed for the current PZ7020 development-hardware scope. Systems 13–15 are deferred.

## Summaries (this set)

| Document | Audience |
| --- | --- |
| [Executive summary](LIGHTSPEED_STAGE1_EXECUTIVE_SUMMARY.md) | executives, partners, investors, engineers |
| [Technical architecture](LIGHTSPEED_STAGE1_TECHNICAL_OVERVIEW.md) | engineers |
| [System 1–12 ledger](LIGHTSPEED_STAGE1_SYSTEM_LEDGER.md) | engineering history |
| [Evidence index](LIGHTSPEED_STAGE1_EVIDENCE_INDEX.md) | auditors, bring-up |
| [Ownership and licensing](LIGHTSPEED_STAGE1_OWNERSHIP.md) | legal / engineering |
| [Security status](LIGHTSPEED_STAGE1_SECURITY_STATUS.md) | security / production |
| [Boot performance](LIGHTSPEED_STAGE1_BOOT_PERFORMANCE.md) | measured timing only |
| [Release status](LIGHTSPEED_STAGE1_RELEASE_STATUS.md) | product status |
| [Production-security deferral](DEFERRED_PRODUCTION_SECURITY_HARDWARE.md) | Systems 13–15 |

## Per-system references

[SYS01](SYS01_WORKING_REFERENCE_LOCK.md) · [SYS02](SYS02_BOOT_IMAGE_REFERENCE.md) · [SYS03](SYS03_PS7_REFERENCE.md) · [SYS04](SYS04_SD_REFERENCE.md) · [SYS05](SYS05_PARSER_REFERENCE.md) · [SYS06](SYS06_P1_DEVCFG.md) · [SYS07](SYS07_RSA_REFERENCE.md) · [SYS08](SYS08_PCAP_REFERENCE.md) · [SYS09](SYS09_HANDOFF_REFERENCE.md) · [SYS10 size](STAGE1_SIZE_REPORT.md) · [SYS11](SYS11_UART_REFERENCE.md) · [SYS12](SYS12_RECOVERY_REFERENCE.md)

## Validation

`python docs/boot/validate_lightspeed_stage1_docs.py` from the repository root. Does not open COM12 or write an SD card.
