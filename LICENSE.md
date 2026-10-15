# Golden Phase Operating System (GPOS) — Dual License

**Copyright © 2024–2026 Richard Careaga, Metatron Research. All rights reserved.**

**Product:** Golden Phase Operating System (GPOS)  
**Former brand:** KairOS (Cargo package name `kairos` retained for stability)  
**Copyright holder:** Richard Careaga, Metatron Research, Thousand Oaks, California  
**SPDX (open-source path):** `AGPL-3.0-only`

This repository is **dual-licensed**. You choose one path. You may not mix the two for the same deployment.

| Path | License | Who it is for |
|------|---------|----------------|
| **A — Open Source** | [GNU Affero General Public License v3.0 only](https://www.gnu.org/licenses/agpl-3.0.html) | Research, students, hobbyists, public audit, community development |
| **B — Commercial / Enterprise** | Proprietary license from the copyright holder | Companies that must keep their software, FPGA IP, or products closed |

The copyright holder is the sole author of the GPOS core and retains **100% copyright** in that core. That ownership is what makes Path B legally possible. Third-party libraries bundled or linked with GPOS remain under **their own licenses** and are not relicensed by this file.

This file is the licensing policy for the repository. It is not a substitute for the full AGPLv3 text (Path A) or a signed commercial agreement (Path B). It is also not legal advice. Organizations with proprietary, defense, or OEM requirements should have counsel review Path A against their product architecture, or purchase Path B.

---

## 1. Why dual licensing

GPOS is a bare-metal, Rust-based, hardware-accelerated operating system targeting ARMv7-A / Xilinx Zynq-7020 and related FPGA platforms. The public copyleft path keeps the core OS auditable and free for research. The commercial path exists because AGPLv3 copyleft is often incompatible with:

- proprietary application software shipped on or with GPOS
- closed FPGA bitstreams, HDL, and board-level IP
- defense, robotics, aerospace, and OEM products that cannot disclose source
- products offered as a network service without releasing corresponding source (AGPLv3 §13)

If those constraints apply to you, Path A is the wrong license. Use Path B.

---

## 2. Path A — Open Source (GNU AGPLv3-only)

Unless you have a written commercial license from Richard Careaga / Metatron Research, GPOS is licensed under the **GNU Affero General Public License, version 3, only** (not “or later”).

The official license text is:

https://www.gnu.org/licenses/agpl-3.0.html

A copy of that text must accompany every AGPL distribution of this software. If this repository includes `COPYING` or `LICENSE-AGPL-3.0.txt`, that file is the binding Path A instrument.

### You may

- run, study, modify, and share GPOS for any purpose, including commercial *use*, subject to AGPLv3
- audit the kernel, protocol, PhaseVM/ISA, and FPGA bring-up code
- publish research, patches, and derivative works under AGPLv3
- use GPOS in teaching, personal projects, and public scientific work
- include GPOS excerpts in academic publications as granted in §4 (this does not replace AGPLv3 for distributing the software itself)

### You must

- preserve copyright, license, and notice files
- license your modifications of GPOS, and any work that is a derivative of GPOS under copyright law, under AGPLv3
- provide **Corresponding Source** to recipients when you **convey** GPOS (devices, images, `BOOT.BIN` / ELF distributions, source tarballs, and the like)
- if you modify GPOS and users **interact with it remotely through a computer network**, offer those users Corresponding Source under AGPLv3 §13
- include installation information when AGPLv3 requires it for User Products

### You must not (under Path A)

- distribute a proprietary product that incorporates GPOS without releasing the required source under AGPLv3
- keep closed the GPOS modifications you ship in a product or network service
- assume that proprietary HDL, bitstreams, board support, or application code combined with GPOS is automatically a “mere aggregation” and therefore exempt — that question is fact-specific. If you cannot open the combined work, obtain Path B
- relicense GPOS under MIT, Apache-2.0, GPL-2.0, a proprietary license, or any terms other than AGPLv3-only

**No warranty.** Path A is provided “AS IS”, without warranty or liability, to the extent permitted by law, as stated in AGPLv3.

### FPGA, devices, and network use

GPOS is designed to boot on real hardware. Conveying a board image, ELF, or `BOOT.BIN` that contains GPOS is distribution. Offering a modified GPOS instance that others interact with over a network (console, SSH, telemetry, mission control, or similar) engages AGPLv3 §13. Path A is the correct license only if you can meet those source obligations.

---

## 3. Path B — Commercial / Enterprise License

A commercial license is a **separate, written agreement** between the licensee and Richard Careaga / Metatron Research. It replaces AGPLv3 for the licensed GPOS code for that licensee’s permitted use.

Path B is the intended route for proprietary commercial, defense, robotics, aerospace, and OEM users who cannot, or will not, open-source their own software or custom hardware IP.

### A commercial license is designed to grant (subject to the signed agreement)

- use of GPOS in closed-source products without AGPLv3 copyleft
- the right **not** to disclose licensee proprietary software, HDL, FPGA bitstreams, board designs, or mission payloads
- production deployment, and where purchased, OEM redistribution in licensee products
- optional support, source access, warranty, and indemnity as specified in the agreement
- optional paid engineering: ports to additional SoCs, FPGA families, BSPs, and board bring-up

### A commercial license does not, by itself

- transfer copyright in GPOS
- relicense third-party components
- grant trademark rights in “Golden Phase”, “GPOS”, “KairOS”, or “Metatron”
- grant rights in future unrelated products
- constitute an export-control, ITAR/EAR, security, or classified-program authorization — those are negotiated separately when required

Pricing, seats, SKUs, deployment caps, support SLAs, and OEM royalties are defined in the commercial agreement, not in this file. See [`COMMERCIAL_LICENSE.md`](COMMERCIAL_LICENSE.md) for the current tier template.

Until a commercial agreement is fully executed, Path A (AGPLv3-only) applies.

The Path B tier template (Startup, Enterprise, Hardware Bundle / OEM) is [`COMMERCIAL_LICENSE.md`](COMMERCIAL_LICENSE.md). That file is part of this licensing set and must be published with this `LICENSE.md`.

---

## 4. Academic and research publication permissions

Researchers, academics, and students are encouraged to audit, benchmark, and publish academic papers, research findings, or technical write-ups using GPOS.

This section is a **copyright permission for scholarly publication**. It does not replace Path A (AGPLv3) for conveying or modifying the software, and it is not a Path B commercial license. It does not grant trademark endorsement, and it does not cover third-party crates or vendor materials.

### Citation and screenshots

You are granted permission to include GPOS code snippets, serial terminal outputs, logs, architecture diagrams, and benchmark results in academic papers, journals, conferences, theses, dissertations, and educational materials **without prior written consent**, provided proper citation is given to **Richard Careaga / Metatron Research**.

Suggested citation (adapt to the venue’s style):

```text
Careaga, R. (2026). Golden Phase Operating System (GPOS) [Computer software].
Metatron Research, Thousand Oaks, CA.
```

Name the git tag or commit SHA when the result depends on a specific build. Do not present research-gated or unsealed FPGA images as production GPOS.

This permission does **not** authorize:

- commercial advertising, product datasheets, or investor materials (use Path B or a separate trademark/copyright request)
- reproducing GPOS in full as a substitute for AGPLv3 source conveyance
- implying that Metatron Research reviewed, funded, or endorses the publication

### Formal permission letters

If an academic publisher or grant funding body requires a formal, written Copyright Permission Letter or a customized waiver for a specific publication, paper, or thesis, contact **licensing@metatronresearch.org**.

Permission is granted **free of charge** for non-commercial academic research. Include the working title, venue, author list, and a description of the GPOS material to be reproduced (figures, listings, logs).

Commercial reprint, paid courseware sold as a product, or publisher-side exclusive rights that would conflict with AGPLv3 or with Path B are not covered by this free grant; those requests are reviewed separately.

---

## 5. Which path applies

```
Do you have a signed commercial license covering this use?
        │
        ├─ Yes ──► Path B (the signed agreement controls)
        │
        └─ No ───► Path A (AGPLv3-only)
                   If you cannot comply with AGPLv3, do not use GPOS
                   in that product until Path B is in place.
```

| Question | Path A (AGPLv3) | Path B (Commercial) |
|----------|-----------------|---------------------|
| Cost for the license itself | None | Fee per signed agreement |
| Must GPOS modifications be open-sourced? | Yes, when you convey or (if modified) offer network interaction | No, unless the agreement says otherwise |
| May licensee proprietary apps stay closed? | Only if they are not a derivative of GPOS; combination risk remains | Yes, within the grant |
| May custom FPGA / HDL / board IP stay closed? | Uncertain under copyleft; do not rely on Path A | Yes, within the grant |
| OEM redistribution in a product | Only with full AGPLv3 compliance | Yes, if the OEM/Hardware Bundle (or equivalent) grant is purchased |
| Warranty / indemnity | None (AGPLv3 disclaimer) | As negotiated |
| Copyright owner | Remains Richard Careaga | Remains Richard Careaga |

---

## 6. Contributions and copyright control

External contributions are accepted **only** under the Contributor License Agreement in [`CONTRIBUTING.md`](CONTRIBUTING.md). Contributors assign copyright to Richard Careaga or, where assignment cannot take effect, grant an irrevocable right to relicense the contribution under any terms, including proprietary commercial licenses.

Do not submit a patch if you cannot make that grant (for example, if your employer owns the work and will not sign). Contributions submitted under AGPLv3-only, without the CLA, **will not be merged**. That rule exists so Path B remains legal.

---

## 7. Third-party and non-GPOS material

GPOS links to and may vendor third-party crates, headers, vendor FSBL/boot artifacts, and FPGA tooling outputs. Those components are **not** dual-licensed by Metatron Research. Their notices and licenses control. A commercial GPOS license does not replace Apache-2.0, MIT, BSD, or other inbound licenses, and it cannot make copyleft inbound code proprietary.

Do not place third-party copyleft code into this tree without prior written approval from the copyright holder. Inbound GPL/AGPL/LGPL code that the holder does not own can destroy the ability to offer Path B.

---

## 8. Trademarks

“Golden Phase Operating System”, “GPOS”, “KairOS”, “Golden Phase”, “Metatron Research”, and related names and logos are trademarks or project identifiers of Richard Careaga / Metatron Research. A copyright license (Path A or Path B) is **not** a trademark license. Do not imply endorsement, certification, or official port status without written permission.

---

## 9. Contact — commercial licenses, academic letters, and custom hardware ports

**Copyright holder:** Richard Careaga  
**Organization:** Metatron Research  
**Location:** Thousand Oaks, California, USA  
**Commercial / engineering:** RichardCareaga@gmail.com  
**Academic permission letters:** licensing@metatronresearch.org

For Path B purchases, OEM terms, defense/aerospace addenda, or paid ports (additional ARM/FPGA targets, BSP/board bring-up, bitstream integration):

1. Read [`COMMERCIAL_LICENSE.md`](COMMERCIAL_LICENSE.md) and identify the tier that matches your use.
2. Email **RichardCareaga@gmail.com** with:
   - legal company name and jurisdiction
   - intended products, SKUs, and deployment scale
   - target hardware (e.g. Zynq-7020, board revision)
   - whether you need OEM redistribution, source escrow, indemnity, or an export-control addendum
   - technical contact and counsel contact
3. Alternatively, open a repository issue or discussion labeled `licensing` **without** attaching ITAR-controlled, classified, or proprietary design files.

Do not send export-controlled technical data through a public issue tracker.

Evaluation use of the public tree remains under Path A until a commercial agreement is signed. If you need a closed-source evaluation, request a time-limited Path B evaluation addendum in the same inquiry.

---

## 10. License notices in source files

Preferred header for GPOS-owned source:

```text
Copyright (c) 2024–2026 Richard Careaga, Metatron Research.
SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
```

`LicenseRef-GPOS-Commercial` means a separate written commercial license from the copyright holder. In the absence of that agreement, `AGPL-3.0-only` applies.
