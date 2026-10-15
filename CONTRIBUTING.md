# Contributing to Golden Phase Operating System (GPOS)

Thank you for considering a contribution. GPOS is dual-licensed: GNU AGPLv3-only for the public, and a proprietary commercial license for organizations that cannot accept copyleft. That model is legal **only** if Richard Careaga remains able to relicense the entire GPOS-owned tree.

**No contribution is merged without a signed Contributor License Agreement (CLA).** Please read this file before writing a substantial patch.

| Document | Role |
|----------|------|
| [`LICENSE.md`](LICENSE.md) | Dual-license policy |
| [`COMMERCIAL_LICENSE.md`](COMMERCIAL_LICENSE.md) | Commercial tier template |
| This file | How to contribute, and the CLA |

---

## 1. Who may contribute

Anyone may propose changes: students, researchers, hobbyists, and industry engineers. You must have the legal right to grant the CLA (you own the work, or your employer has authorized the grant).

Publishing a paper, thesis, or educational write-up about GPOS does **not** require a CLA. See [`LICENSE.md`](LICENSE.md) §4. A CLA is required only when you submit code, documentation, or other material for inclusion in this repository.

If you are employed, check your IP agreement **before** opening a pull request. Many employment contracts assign inventions to the employer. In that case the **employer** must sign the Corporate CLA.

---

## 2. What we welcome

High-value contributions include:

- bug fixes with a reproducible test or hardware log
- documentation that matches actual behavior
- ports and board-support that the copyright holder can own under the CLA
- performance, safety, and auditability improvements
- tests that fail on the bug and pass on the fix

We generally do not accept:

- drive-by refactors with no behavioral change
- new dependencies without a license review
- inbound copyleft code the project does not own (GPL, AGPL, LGPL, MPL in a form that blocks Path B)
- features that expand GPOS into a general-purpose desktop OS
- patches that weaken honesty gates, sealed-boot invariants, or test evidence
- submissions you cannot CLA

If you are unsure, open an issue first and describe the change. Do not attach export-controlled or proprietary vendor files to a public issue.

---

## 3. How to submit

1. **Open an issue** (unless the fix is trivial) describing the problem, target (`hosted` / QEMU / Zynq-7020), and how you will test.
2. **Sign the CLA** (Section 6). Individual contributors sign the Individual CLA. If the work is owned by an organization, that organization signs the Corporate CLA and names you as an authorized engineer.
3. **Fork and branch** from the current integration branch named by maintainers.
4. **Keep the change scoped.** One concern per pull request.
5. **Test what you changed.** Hosted tests must pass. Hardware changes need a log, hash, or evidence note consistent with existing FPGA evidence practice — not a claim without a trace.
6. **Open a pull request** that includes the **CLA / DCO sign-off block** in Section 3.1. Pull requests without that block, or without a CLA on file, will not be merged.
7. **Respond to review.** Maintainers may request tests, smaller diffs, or license clarifications.

---

## 3.1 Pull request CLA and sign-off (required)

Every pull request must contain the following block in the PR description. Replace the bracketed fields. The `Signed-off-by` line must match the commit trailer (`git commit -s`).

```text
CLA: I have read CONTRIBUTING.md Section 6 and I (or my employer under the
Corporate CLA) have signed the GPOS Contributor License Agreement.

I have the legal right to submit this work. This pull request does not include
secrets, private keys, or export-controlled technical data.

Signed-off-by: Full Legal Name <email@example.com>
```

**Merge rules**

- No CLA signature (Individual or Corporate) on file → **do not merge**.
- No `Signed-off-by` on the pull request **and** on each commit → **do not merge**.
- `Signed-off-by` identity must match the CLA record (name and email).
- Opening a pull request constitutes agreement to Section 6 (assignment / fallback relicensing grant, patent license, and DCO certifications) for every file in the PR.

---

## 4. Coding and review expectations

- Match the style of the file you edit. Do not reformat unrelated code.
- Do not add abstraction “for later.” GPOS prefers concrete, testable changes.
- New `unsafe` Rust requires a comment stating the invariant.
- Feature flags that affect sealed boot default to **off**, consistent with existing crate features.
- Third-party crates require SPDX/license identification and copyright-holder approval if the license is not permissive (MIT, Apache-2.0, BSD-2/3, ISC, or equivalent).
- Do not add `or later` AGPL language, additional licenses, or CLA exceptions in source headers.

Review is technical and legal. A patch that is correct in engineering terms can still be rejected for license, CLA, or inbound-IP reasons.

---

## 5. Why a CLA is required (plain language)

Under ordinary open-source contribution, **you** own copyright in your patch. The copyright holder could not then offer GPOS under a commercial license that includes your patch, unless you granted that right.

The CLA does two things:

1. **Copyright assignment** to Richard Careaga, so one person can license the core under AGPLv3 and under commercial terms.
2. **Fallback relicensing grant** — if assignment is not effective in your jurisdiction, you still grant an irrevocable, worldwide, perpetual, royalty-free license that includes the right to relicense under any terms, including proprietary commercial licenses.

You receive a **license back** to your contribution so you can continue to use your own work. You are not selling the patch; you are enabling the dual-license model that funds and protects the project.

GPOS will continue to be available to the public under AGPLv3. The CLA does not take the public tree proprietary. It lets the same tree also be licensed commercially to parties who cannot use AGPLv3.

---

## 6. Contributor License Agreement (CLA)

**Version 1.0 — 13 August 2026**  
**Project:** Golden Phase Operating System (GPOS / formerly KairOS)  
**Assignee / Grantee:** Richard Careaga, Metatron Research, Thousand Oaks, California (“**Copyright Holder**”)

This Section 6 is the operative CLA text. A contribution is not accepted until the Copyright Holder has a signed copy (electronic signature or CLA-bot record is sufficient) and the certifications below.

Replace bracketed fields before signing.

### 6.1 Definitions

- **“You”** — the individual or legal entity signing.
- **“Contribution”** — any original work of authorship, including software, documentation, tests, designs expressed in source form, and modifications, that You submit to the GPOS project in any form (pull request, patch, issue attachment, email, or otherwise), excluding material You explicitly mark “Not a Contribution.”
- **“Submit”** — any form of electronic, verbal, or written communication sent to the project for the purpose of inclusion.

### 6.2 Copyright assignment (primary grant)

To the maximum extent permitted by applicable law, You hereby **irrevocably assign** to the Copyright Holder all right, title, and interest in and to the Contribution, including all copyrights, copyright renewals and extensions, and all causes of action for past, present, or future infringement.

The Copyright Holder may register copyright, enforce copyright, and license the Contribution under:

- GNU AGPLv3-only (or any open-source license the Copyright Holder later chooses for the public tree), and
- any commercial, proprietary, or other license, including dual licensing,

without further consent from You and without royalty to You.

### 6.3 Fallback license (if assignment cannot take effect)

If and to the extent that assignment in Section 6.2 is invalid or unenforceable in a relevant jurisdiction, You hereby grant the Copyright Holder an **irrevocable, perpetual, worldwide, exclusive, royalty-free, fully paid-up, transferable, and sublicensable** license under all of Your copyright and related rights in the Contribution to:

- reproduce, prepare derivative works of, distribute, publicly perform, publicly display, and otherwise use the Contribution in any manner and media;
- **relicense** the Contribution, alone or as part of GPOS, under **any licensing terms whatsoever**, including GNU AGPLv3-only and proprietary commercial licenses, without copyleft, source-disclosure, or share-alike obligations to You;
- exercise these rights through multiple tiers of sublicensees (including commercial customers).

This fallback license is intended to be **Harmony CLA outbound option 5**-equivalent: outbound terms are at the Copyright Holder’s sole discretion. It is exclusive to the Copyright Holder except for the license-back in Section 6.5.

### 6.4 Patent license

You grant the Copyright Holder and all licensees of GPOS (under AGPLv3 or under a commercial license) a perpetual, irrevocable, worldwide, royalty-free, non-exclusive patent license to make, have made, use, sell, offer to sell, import, and otherwise transfer the Contribution and derivative works thereof, where such license applies only to those patent claims You own or control that are necessarily infringed by Your Contribution alone or by combination of Your Contribution with GPOS.

If You institute patent litigation against any entity alleging that the Contribution or GPOS constitutes direct or contributory patent infringement, any patent licenses granted under this CLA to You for that work terminate as of the filing date of such litigation.

### 6.5 License-back to You

The Copyright Holder grants You a perpetual, irrevocable, worldwide, royalty-free, non-exclusive license to use, modify, and distribute Your Contribution, including as part of Your other work. This license-back does **not** grant You the right to relicense GPOS as a whole, to grant commercial GPOS licenses to third parties, or to use Metatron / GPOS trademarks except as allowed by [`LICENSE.md`](LICENSE.md).

### 6.6 Moral rights

To the extent permitted by law, You waive (or agree not to assert against the Copyright Holder or its licensees) moral rights in the Contribution, including rights of attribution and integrity, solely as needed for the Copyright Holder to exercise the rights in this CLA. Where waiver is not permitted, You consent to the Copyright Holder’s reasonable modification, translation, and relicensing of the Contribution.

### 6.7 Your certifications (DCO + authority)

You certify that:

1. The Contribution is Your original creation, or You have sufficient rights from the owner to make the grants in this CLA.
2. Each Contribution is accompanied by the following statement (or the pull request is understood to include it):

   ```text
   Signed-off-by: Your Name <your.email@example.com>
   ```

   meaning You agree to the Developer Certificate of Origin, version 1.1  
   (https://developercertificate.org/).
3. You have the legal authority to sign. If the Contribution was created in the course of employment, Your employer has signed the Corporate CLA or You have written authorization to sign.
4. The Contribution does not include material You know to be confidential, export-controlled in a manner that forbids this public submission, or licensed under terms that would prevent dual licensing of GPOS.
5. You will notify the Copyright Holder if any of these certifications becomes inaccurate.

**AI-assisted contributions.** If You used a code-generation tool, You still make every certification above. You must have the right to assign or relicense the submitted text. Do not submit output that you know is copied from a third-party codebase that you cannot CLA.

### 6.8 No obligation to use

The Copyright Holder is not obligated to include any Contribution. Unused Contributions remain subject to this CLA if they were Submitted.

### 6.9 Governing law

This CLA is governed by the laws of the State of California, USA, excluding conflict-of-law rules, unless a different governing law is required by a mandatory local statute that cannot be waived. The United Nations Convention on Contracts for the International Sale of Goods does not apply.

### 6.10 Entire agreement; counterparts

This CLA is the entire agreement regarding Contributions. It may be executed electronically. Headings are for convenience only.

---

## 7. Signature blocks

### Individual CLA

```text
Golden Phase Operating System (GPOS) — Individual Contributor License Agreement v1.0

I have read CONTRIBUTING.md Section 6 and agree to its terms.

Full legal name:     ________________________________
GitHub / Git identity: ________________________________
Email:               ________________________________
Date (YYYY-MM-DD):   ________________________________
Signature:           ________________________________

I am signing on my own behalf. I am not transferring rights I do not own.
```

### Corporate CLA

```text
Golden Phase Operating System (GPOS) — Corporate Contributor License Agreement v1.0

The undersigned legal entity has read CONTRIBUTING.md Section 6 and agrees
to its terms for all Contributions submitted by its authorized employees
and contractors listed (or later listed in writing).

Legal entity name:   ________________________________
Jurisdiction:        ________________________________
Address:             ________________________________
Authorized signatory: ________________________________
Title:               ________________________________
Email:               ________________________________
Date (YYYY-MM-DD):   ________________________________
Signature / company stamp: ___________________________

Authorized engineers (name, email, Git identity):
1. ________________________________
2. ________________________________

The entity warrants it owns or controls the Contributions and has authority
to make the assignment and licenses in Section 6.
```

Send signed CLAs to **RichardCareaga@gmail.com**, or complete the project’s CLA-bot / CLA Assistant flow when enabled. Maintainers will not merge until the record is on file.

---

## 8. Security and sensitive reports

Do not file public issues for:

- cryptographic key material or credentials
- unpatched remotely exploitable flaws you believe are still live on fielded boards
- ITAR/EAR-controlled technical data
- classified or proprietary customer designs

Email **RichardCareaga@gmail.com**. Do not attach controlled technical data until a private channel is confirmed.

---

## 9. Conduct

Be precise. Disagree on facts and patches, not on people. Harassment, legal threats in review threads, and drive-by license bikeshedding will get a contribution closed. Substantive license questions belong in an issue labeled `licensing` or in counsel-to-counsel correspondence.

Questions about using GPOS in a proprietary product belong in Path B (`COMMERCIAL_LICENSE.md`), not in a request to weaken AGPLv3.
