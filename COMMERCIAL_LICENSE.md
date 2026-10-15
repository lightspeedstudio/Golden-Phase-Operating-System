# GPOS Commercial License — Tier Template

**Product:** Golden Phase Operating System (GPOS)  
**Licensor:** Richard Careaga, Metatron Research, Thousand Oaks, California  
**Document type:** Commercial tier summary and term-sheet template  
**Status:** Template — **not** an executable contract and **not** a quote  
**Related:** [`LICENSE.md`](LICENSE.md) (dual-license policy) · [`CONTRIBUTING.md`](CONTRIBUTING.md) (CLA)

This document describes typical Path B packages for proprietary, OEM, robotics, aerospace, and defense buyers. Binding terms exist only in a signed agreement. Fees, caps, and SLAs below are **structural placeholders** (`Quoted`) so legal and procurement teams can map internal approval paths. They are not list prices.

Until a commercial agreement is executed, GPOS remains under **GNU AGPLv3-only** ([`LICENSE.md`](LICENSE.md)).

---

## 1. What Path B is for

Buy Path B when you need to:

- ship GPOS in a closed-source product
- keep proprietary applications, mission software, HDL, FPGA bitstreams, or board IP unpublished
- avoid AGPLv3 source-disclosure and §13 network-copyleft
- obtain support, a hardware port, or contractual warranty/indemnity
- redistribute GPOS in OEM devices under your trademark and SKU

Do **not** buy Path B if AGPLv3 already covers your use (public research, in-house experiments you can open-source, teaching). Path A is free. Academic papers, theses, and educational write-ups that quote GPOS snippets, logs, diagrams, or benchmarks are covered by the free publication grant in [`LICENSE.md`](LICENSE.md) §4 — not by a commercial tier. Formal publisher/funder letters: **licensing@metatronresearch.org**.

---

## 2. Tier summary

| | **Startup** | **Enterprise** | **Hardware Bundle / OEM** |
|---|---|---|---|
| **Typical buyer** | Incorporated company below an agreed headcount and revenue cap; early product | Established commercial, industrial, or government-prime organization | Device, vehicle, or board vendor shipping GPOS in a product SKU |
| **License grant** | Non-exclusive, non-transferable (except to successor in a qualifying change of control, as agreed) | Same, with broader deployment and affiliate language | Same, plus a defined **redistribution** grant for Licensed Products |
| **Copyleft** | **Waived** for licensed GPOS code in Authorized Use | **Waived** | **Waived**, including binary/board-image distribution within the SKU schedule |
| **Must open licensee IP?** | No | No | No |
| **Internal use / R&D** | Yes | Yes | Yes |
| **Production deployment** | Limited units / sites (schedule) | Uncapped or high cap (schedule) | Per Licensed Product / unit band |
| **Redistribution to customers** | Not included (add OEM or upgrade) | Optional rider | **Included** for named SKUs |
| **FPGA / HDL of licensee** | Licensee retains; no disclosure to Licensor required | Same | Same; Licensor may require interface specs only for paid porting |
| **GPOS source access** | Optional (Quoted) | Standard or escrow (Quoted) | Standard for manufacturing/support; escrow optional |
| **Support** | Email, best effort, business days | Contracted SLA (response/restore) | SLA + designated engineering contact |
| **Warranty** | Limited, as-is or short warranty (Quoted) | Negotiated warranty | Negotiated; may include board-bring-up acceptance tests |
| **IP indemnity** | Not included or cap-limited (Quoted) | Standard commercial indemnity with caps/exclusions | Standard, plus OEM downstream flow-down language |
| **Custom hardware port** | Quoted separately | Discounted or included allowance (Quoted) | **Bundle candidate:** Zynq-7020 BSP, additional SoC/FPGA family, FSBL/`BOOT.BIN` pipeline |
| **Defense / aerospace** | Not a substitute for an addendum | Addendum available | Addendum available (export, audit, longevity) |
| **Trademark use** | Factual “powered by GPOS” only if approved | Logo use by written brand schedule | OEM may use GPOS marks only as in the brand schedule; no certification implied |
| **Term** | Annual or 3-year (Quoted) | Multi-year / perpetual + maintenance (Quoted) | Perpetual for a frozen major.minor **or** term + maintenance (Quoted) |
| **Fee model (structure)** | Annual subscription | Annual or prepaid perpetual + yearly maintenance | Prepaid license + **per-unit** or **band** royalty, or royalty-buyout |
| **Price** | Quoted | Quoted | Quoted |

**Startup eligibility** is defined in the agreement (example structure: ≤ 20 employees **and** trailing-12-month revenue below a stated cap, non-affiliate of a larger enterprise). Outgrowing the cap requires conversion to Enterprise or OEM at a stated conversion formula. Startups that are majority-owned by an ineligible parent are not Startup-tier.

---

## 3. Grant language (template — to be negotiated)

The signed license should contain a grant substantially of this form:

> Subject to timely payment and this Agreement, Licensor grants Licensee a non-exclusive license to use, modify (for Authorized Use), compile, load, and run the GPOS software identified in Exhibit A, in source and object form, **without** the copyleft, source-conveyance, or network-disclosure obligations of the GNU AGPLv3, solely for Authorized Use. Licensee is **not** required to disclose Licensee proprietary software, hardware description language, FPGA bitstreams, board designs, or data by reason of combination with GPOS.

**Authorized Use** is tier-specific (internal, production, OEM SKU list). **Licensed Products** (OEM) are named in a schedule (model, SKU, FPGA part).

**Reserved rights.** Copyright remains with Richard Careaga. No right is granted to offer GPOS as a competing dual-licensed or open-source distribution, to remove copyright notices from GPOS files, or to use the license as a substitute for inbound third-party licenses.

---

## 4. Hardware Bundle / OEM (detail)

The Hardware Bundle / OEM tier is the intended fit for companies that ship a physical or fielded system (robot, craft, instrument, industrial controller, or Zynq-class SOM/board).

Typical exhibit items:

| Exhibit item | Content |
|--------------|---------|
| A — Software | GPOS version (git tag / SHA), permitted feature flags, excluded research flags |
| B — Licensed Products | SKU list, processor (e.g. Zynq-7020), board name, FPGA family |
| C — Redistribution | Right to convey object form (ELF, `BOOT.BIN`, flash image) to end customers of Licensed Products only |
| D — Porting SOW | Paid engineering: pin mux, DDR, UART/JTAG console, GEM/USB, bitstream slot, boot chain |
| E — Updates | Security and maintenance branch duration (e.g. N years from acceptance) |
| F — Units | Royalty band or buyout; reporting period; audit |

OEM redistribution applies to **Licensee products**, not to a general right to sell GPOS as a standalone OS.

Custom porting is a statement of work, not a license grant. Acceptance should be tied to written tests (boot log, hash of image, console evidence), not to marketing claims.

---

## 5. Defense, robotics, and aerospace addendum (optional)

Not a separate public tier. Attached to Enterprise or OEM when required.

Typical clauses to negotiate (include only what the program needs):

- **Export control.** Licensee is importer/exporter of record for its product. Licensor does not accept ITAR technical data through public repositories. Controlled drawings use a marked, private channel.
- **No telemetry.** GPOS commercial builds can be specified with no phoning-home; Licensee is responsible for its own radios and links.
- **Longevity.** Multi-year freeze of a named release plus paid maintenance; optional source escrow with a neutral agent and release conditions (bankruptcy, support cessation).
- **Audit of license counts**, not of Licensee mission IP.
- **Flow-down** to primes/subs: Licensee may sublicense object form to a named prime only as needed to deliver Licensed Products, without granting that prime a GPOS development license.
- **Classified programs.** GPOS is not a classified product. Any classified integration is Licensee’s environment; Licensor personnel are not cleared unless a separate consulting agreement says so.
- **Safety / DO-178 / ISO 26262 / IEC 61508.** Path B does **not** assert certification. Certification artifacts, if any, are a separate SOW.

---

## 6. What every commercial tier excludes

Unless an exhibit expressly adds them:

- copyright transfer
- patents except as licensed for use of the delivered GPOS version
- trademarks beyond the brand schedule
- third-party crate/vendor licenses (Apache, MIT, boot ROM, Xilinx/AMD tools, etc.)
- GPL/AGPL inbound code that is not owned by Licensor
- unlimited custom engineering
- open-ended “or later” rights to unreleased GPOS major versions (updates are a maintenance exhibit)
- any right to relicense GPOS to third parties as OS vendors

---

## 7. Evaluation

| Mode | Terms |
|------|--------|
| Public tree | AGPLv3-only. Suitable if evaluation artifacts can be public or you accept copyleft. |
| Closed evaluation | Time-limited Path B evaluation addendum (typical structure: 30–90 days, non-production, no OEM, no indemnity). Convert to a paid tier or cease proprietary use at end of term. |

---

## 8. Procurement checklist (for licensee counsel)

1. Confirm you **cannot** or **will not** comply with AGPLv3 for the intended product. If you can comply, Path A is sufficient.
2. Map combination of GPOS with closed HDL/software; choose Enterprise vs OEM accordingly.
3. List target SKUs, FPGA parts, and unit forecast (OEM).
4. Identify affiliates that need the grant.
5. Decide source-access vs escrow.
6. Flag export-control and longevity needs.
7. Do not send controlled technical data in the first email; request a channel.
8. Plan inbound-license review of GPOS third-party dependencies in parallel with the GPOS grant.

---

## 9. How to request a quote

Email **RichardCareaga@gmail.com**. Academic permission letters: **licensing@metatronresearch.org**. See also [`LICENSE.md`](LICENSE.md) §9.

Send a non-confidential summary:

- requested tier (Startup / Enterprise / Hardware Bundle / OEM)
- legal entity, jurisdiction, and whether a parent company exists
- product description at unclassified/public level
- hardware targets (e.g. Xilinx Zynq-7020, board name)
- production vs R&D; expected units/year if OEM
- support, indemnity, porting, and addenda required
- desired term (annual, multi-year, perpetual + maintenance)

Licensor will return a draft agreement and a fee schedule. **No license is granted by email, invoice intent, or use of this template.**

---

## 10. Term-sheet fields (copy into the negotiated cover page)

```text
Licensor:     Richard Careaga, Metatron Research
Licensee:     [legal name], [jurisdiction]
Tier:         [ ] Startup  [ ] Enterprise  [ ] Hardware Bundle / OEM
Effective:    [YYYY-MM-DD]
Term:         [ ]
GPOS version: [git tag / SHA]
Authorized Use: [ ]
Licensed Products / SKUs: [ ]   (OEM)
Unit band / royalty / buyout: [ ]   (OEM)
Support SLA:  [ ]
Source:       [ ] none  [ ] licensed access  [ ] escrow
Porting SOW:  [ ] none  [ ] attached as Exhibit D
Defense/aerospace addendum: [ ] no  [ ] attached
Fee:          [Quoted]
Governing law: [California, USA, unless otherwise agreed]
```

---

## 11. Relationship to AGPLv3

Path B is an alternative license from the copyright owner, not a “waiver exception” patched onto AGPLv3. Licensee does not need to comply with AGPLv3 for GPOS code covered by a valid commercial agreement. If the agreement ends and Licensee continues to use GPOS, use must stop or revert to full AGPLv3 compliance (including source obligations for any conveyed or network-offered modified versions).

Contributions from third parties are included in Path B only when they are under the GPOS CLA ([`CONTRIBUTING.md`](CONTRIBUTING.md)). Code that was never CLA-assigned is not offered under this template.
