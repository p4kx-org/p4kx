# Independent extraction and media tool comparison

Research date: 2026-11-06. P4KX baseline:
[1ee9c0edbfc34e616d2f140951e4eb5bc1c67558](https://github.com/p4kx-org/p4kx/tree/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558).

This extends the [PTU media audit](ptu-media-compatibility-audit.md) and
[catalog audit](ptu-export-compatibility-audit.md). Findings contain technical structures,
coverage and synthetic examples, not game assets, keys, private PTU identities or gameplay values.

## Outcome

[Overview #1103](https://github.com/p4kx-org/p4kx/issues/1103) tracks ten new issues in
milestone v0.5.0: one overview, four bugs and five export/decoder enhancements.
Existing #1093, #1097, #1100 and #1101 were extended with source evidence instead of duplicated.
The common provider plan #1089 and media overview #1088 were updated consistently.

The installed-PTU raw/XML/DataForge comparisons found no unexplained P4KX discrepancy.
New defects concern differential-verification trust and synthetic supported-v2 Archive reading.
The installed PTU uses v1; no actual PTU v2 data loss is claimed.

## Sources and exact scope

Detailed pinned evidence, source seams, licences and capability matrices are in:

- [unp4k/unforge audit](reference-unp4k-audit.md).
- [StarBreaker audit](reference-starbreaker-audit.md).
- [Additional tools audit](reference-other-tools-audit.md).

| Reference | Pinned scope |
| --- | --- |
| [unp4k/unforge](https://github.com/dolkensp/unp4k/tree/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b) | v4.0.87 source and manual PTU raw/XML production. A private net8 build disabled automatic HTTP exception reporting only; extraction/conversion code was unchanged. |
| [StarBreaker release](https://github.com/diogotr7/StarBreaker/tree/450fa284d4b1cbed6832675297dfb02579c0abd3) | Official v0.3.2 CLI, release ZIP hash verified, DataForge plus bounded model/DDS exports. |
| [StarBreaker main](https://github.com/diogotr7/StarBreaker/tree/7d3f01e76307354bbc56a64a70eb5167b1189b64) | Separately inspected current source. Same version string does not imply release parity; the release lacks --all. |
| [Svarog](https://github.com/19h/svarog/tree/2e46eff4f7d475b8826ccfdc01c4a95db764db0d) | Direct Archive/DataCore source; independent synthetic fixtures exercised P4KX, not Svarog runtime. |
| [scdatatools](https://gitlab.com/cpt_ba/scdatatools/-/tree/972b2ed899278fe1a9e8c44747a2a87e1c21246c) / [StarFab](https://gitlab.com/cpt_ba/starfab/-/tree/d498946d06784cfd22ee273b5dde4069f84af58b) | Historical direct-reader/blueprint source plus publisher scdatatools 1.0.4 release. GitLab retrieval was partly limited; no PTU5 runtime-parity claim. |
| [Cryengine-Converter](https://github.com/Markemp/Cryengine-Converter/tree/7b951dbb254ab99f560e8ffbf64a771c70ccef74) / [CryBlend](https://github.com/Tzeentchnet/CryBlend/tree/48b9b6c4a4f4c4dded1f2118eb172a5ff7a7eccc) | Extracted-asset/Blender source comparison. No direct P4K reader parity assumed. |
| [wwiser](https://github.com/bnnm/wwiser/tree/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3) / [vgmstream](https://github.com/vgmstream/vgmstream/tree/6802153425adde134e49394fc653455166d7d2f2) | Adjacent audio-format sources; no P4K reader or acoustic runtime comparison claimed. |

Archived unp4k_rs and scorg-tools/Blender-Tools were additional discovery controls.
No foreign implementation was ported; source age, licence and runtime limits remain explicit.

## Executed PTU comparisons

Fresh P4KX release binaries used locked offline dependencies. Foreign tools ran manually outside
tests/CI. All private output stayed in the audit task directory outside the repository.

| Check | Result |
| --- | --- |
| unp4k raw DCB/chrparams/CDF | 5,921 compared, byte-identical; zero differences or allowances. |
| unp4k bounded media | 34 compared, byte-identical: 6 Stored/plain, 26 method100/plain, 2 method100/encrypted. Includes geometry, textures, bank/WEM, socpak and CAF/DBA/AIM. |
| unforge isolated chrparams | 4,209 trees equal. |
| unforge isolated CDF | 1,711 trees equal. |
| StarBreaker release | 65,321 Record files: 64,419 equal and 902 items with only established allowances; zero unexplained differences. |
| Embedded Record content | 55,953 nested/embedded Records compared. |
| Allowed StarBreaker values | 1,954 dangling-reference nulls and 48 derived-type weak-pointer nulls, explicitly counted. |
| P4KX all-Record self-check | All 121,224 trees build. 55,903 have no standalone release reference file; this is not an independent all-Record-file comparison. |

Producer output counts/errors and source-family coverage were checked independently of exit status.

## Verification trust defects

Configured empty reference directories make all three comparisons pass with zero compared.
Building P4KX's own trees does not replace independent evidence.

Mixed unforge output has 116 distinct same-stem CDF/chrparams collisions. Its 5,804 output paths
do not cover all 5,920 original sources independently. Separate CDF and chrparams folders cover
the full set. The CLI's XML-filter special case can also emit an additional DCB; actual outputs
must be reconciled with intended selection.

An unchanged-function synthetic harness removes genuine zero suffixes from encrypted Stored,
Deflate, real Zstd and method100 pass-through references. Current allowance accepts every case.
The effective encoded/decompressed context is required; declared method/encryption alone is too broad.

These are #1104/#1105. Tests remain unable to launch/download foreign tools; new manifests and
mutation cases must enforce source provenance, selection, collisions and completeness.

## Media findings and oracle limits

Both tools produced bounded root-only LOD2/mip4 textured Entity GLBs. P4KX passes the repository
checker. StarBreaker parses as GLB but hits the stronger no-orphan-textures invariant; this is
not proven core glTF invalidity or a P4KX defect. Different suppression/material policies prevent
using geometry counts or binary GLB equality as an unconditional oracle.

A 4x4 DDS canary agrees exactly. A 256x256 BC1 representative has identical alpha and at most
one 8-bit RGB level difference. This is a bounded numerical difference, not established decoder
failure or in-game visual equivalence.

An independent raw CAF/header/key probe covers every current PTU CAF:

| Candidate | Matching count / 54,391 |
| --- | --- |
| End-frame word at byte16 equals last key | 54,391 |
| Alternative byte12 word equals last key | 562 |
| Quaternion window at byte20 has unit norm within tolerance | 54,391 |
| Alternative quaternion window at byte16 | 1,321 |

This corroborates P4KX's current layout; an alternative converter interpretation is unsuitable here.
Current StarBreaker also agrees with planar 0xC2 and bitmap-time structure, but its fallback timing
and decoder behavior make it an imperfect strict oracle. #1093 now requires original packed
controller words and complete version-qualified clip metadata with independently checked offsets.

Actual multi-material root attributes/PublicParams/other blocks are discarded before export.
#1097 now covers reader/data-model preservation, not merely a larger JSON DTO. A source default
material differs from the host fallback, but that alone does not prove the game's fallback policy.
No rendering-bug ticket was invented. All 26,307 current MTLs have zero MatRef elements, so that
historical lead was not promoted.

## Additional source data

Nested .pak containers contain compiled shader/cache families and 62 source .ext definitions.
Two bounded grammar samples confirm Version/Property, masks, labels/descriptions, dependencies
and Hidden syntax. #1109 retains unknown/source syntax and separates vocabulary export from
shader execution and numeric-mask rendering. The two samples are not claimed as a complete grammar
census. Compiled .cfib/.cfxb/.shcb remain explicitly opaque.

Populated PTU HIRC families plus independent source establish typed bus/aux-bus, attenuation and
effects/modulators/device work. These build on raw retention #1099 and shared parsing #1100.
State/ActorMixer and DialogueEvent decisions were added to #1100/#1101.

## Synthetic supported-v2 findings

| Fixture | Current P4KX result |
| --- | --- |
| Two base Entries | Both payloads listed/read/CRC-verified. |
| Identical bytes, split trailer1+1 base/extension | Only first Entry returned, zero issues. |
| Extension count with zero extension CDR size | Silently accepted base subset. |
| Complete control with incomplete status bit0 | Opens with zero completeness diagnostic. |

A separate layout checker validates the control/extension structures and flags the malformed case.
The installed PTU is v1, with Stored/plain, method100/plain and method100/encrypted Entries.

#1108 preserves Archive timestamps/signatures/extra fields, install/layout ranges, progress and
stored digest slots without labeling unverified fields verified. #645 remains separate.

## New issues

| Issue | Work |
| --- | --- |
| [#1104](https://github.com/p4kx-org/p4kx/issues/1104) | Reference tests: require source manifests and detect empty, colliding or incomplete output |
| [#1105](https://github.com/p4kx-org/p4kx/issues/1105) | Reference tests: restrict encrypted zero-tail allowances to effective pass-through decoding |
| [#1106](https://github.com/p4kx-org/p4kx/issues/1106) | Archive v2: read and validate extension directory entries and name bytes |
| [#1107](https://github.com/p4kx-org/p4kx/issues/1107) | Archive v2: enforce declared installation completeness and preserve status |
| [#1108](https://github.com/p4kx-org/p4kx/issues/1108) | Archive: export source layout, timestamps, signatures and installation metadata |
| [#1109](https://github.com/p4kx-org/p4kx/issues/1109) | Media: export shader-extension declarations and material flag vocabulary |
| [#1110](https://github.com/p4kx-org/p4kx/issues/1110) | Audio: decode and export Wwise bus routing and ducking source objects |
| [#1111](https://github.com/p4kx-org/p4kx/issues/1111) | Audio: decode and export Wwise attenuation cones and curves |
| [#1112](https://github.com/p4kx-org/p4kx/issues/1112) | Audio: decode and export Wwise effects, modulators and audio devices |

Archive/reference tasks are native children of #1103. Shader/audio tasks remain native children
of #1088 and are cross-linked. All new issues use v0.5.0 and ready-for-agent, with native dependencies.

## Limits and completion

Not every discovered tool was executed; source-only, historical and indirect format tools are
identified separately. No running-game render/acoustic ground truth was collected.

ZIP methods93/101, legacy compiled morphs and PBXML were not promoted to current PTU defect
claims without observed need. Declined old-DataForge support and optional filesystem mounting
were not reissued as new extraction-data requirements.

No implementation code was fixed or committed. The reports and fully specified GitHub issues
are the deliverables. Temporary clones, builds, probes, reference files and game assets are
removed after verification.
