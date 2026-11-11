# Additional reference-tool audit

Research date: 2026-11-06. P4KX source baseline:
[1ee9c0edbfc34e616d2f140951e4eb5bc1c67558](https://github.com/p4kx-org/p4kx/tree/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558).

This source audit complements the unp4k/StarBreaker comparisons and the
[installed PTU media audit](ptu-media-compatibility-audit.md). It distinguishes independent
Archive readers from tools that consume previously extracted assets. Public source was read;
no downloaded third-party executable, installer, Blender extension or package was executed.
One small probe built P4KX's own Archive reader and exercised independently encoded synthetic
Archives. No game credentials, extracted game assets or Archive keys are included in this note.

## Findings

Two v2 Archive behaviours are reproduced with synthetic input: extension CDR entries are
silently omitted, and the incomplete-status bit is silently accepted. Source-level comparison
also identifies Archive layout/install metadata and shader-extension declarations as separate
export families worth evaluating. Neither is covered by exporting model chunks or raw MTL
libraries. The independent main-audit inventory identifies the installed PTU as v1, with Store and zstd
Entries only. The synthetic v2 defects concern another explicitly supported layout; they are
not installed-PTU extraction failures.

No claim that an entire competitor is more correct follows from its README. In particular,
Cryengine-Converter/CryBlend disagree with P4KX on important animation layouts, and both have
explicit limitations. Independent byte-level checks below resolve the current-layout disagreements.

The follow-ups are published as [#1106](https://github.com/p4kx-org/p4kx/issues/1106),
[#1107](https://github.com/p4kx-org/p4kx/issues/1107),
[#1108](https://github.com/p4kx-org/p4kx/issues/1108) and
[#1109](https://github.com/p4kx-org/p4kx/issues/1109), under milestone **v0.5.0** and the
[independent-tool overview #1103](https://github.com/p4kx-org/p4kx/issues/1103).

## Pinned sources and licences

| Project | Pin inspected | Role and licence facts |
| --- | --- | --- |
| [Svarog](https://github.com/19h/svarog/tree/2e46eff4f7d475b8826ccfdc01c4a95db764db0d) | `2e46eff4f7d475b8826ccfdc01c4a95db764db0d`, 2026-06-01 | Direct Rust P4K/DataCore reader and writer. `Cargo.toml` declares MIT; no standalone licence file is present in the pinned owning tree. Facts were compared; no implementation was copied. |
| [Cryengine-Converter](https://github.com/Markemp/Cryengine-Converter/tree/7b951dbb254ab99f560e8ffbf64a771c70ccef74) | `7b951dbb254ab99f560e8ffbf64a771c70ccef74`, 2026-06-21 | Extracted CryEngine/Star Citizen asset converter; owning `LICENSE` is GPL-2.0. No code was imported. |
| [CryBlend](https://github.com/Tzeentchnet/CryBlend/tree/48b9b6c4a4f4c4dded1f2118eb172a5ff7a7eccc) | `48b9b6c4a4f4c4dded1f2118eb172a5ff7a7eccc`, manifest 0.1.6, 2026-05-22 | Blender importer. Its `LICENSE` says MIT and describes a Python port of Cryengine-Converter; the upstream Converter licence remains a separate fact. No code was imported. |
| [scdatatools](https://gitlab.com/cpt_ba/scdatatools/-/tree/972b2ed899278fe1a9e8c44747a2a87e1c21246c) | Public `devel` head `972b2ed899278fe1a9e8c44747a2a87e1c21246c`, 2024-04-23; source release 1.0.4 also inspected | Direct historical P4K/DataCore toolkit plus blueprint/Blender utilities. MIT. GitLab source retrieval was rate-limited after partial reads; detailed fallback inspection used the publisher's complete 1.0.4 source release, not an unverified mirror. |
| [StarFab](https://gitlab.com/cpt_ba/starfab/-/tree/d498946d06784cfd22ee273b5dde4069f84af58b) | Public `devel` head `d498946d06784cfd22ee273b5dde4069f84af58b`, 2023-10-20 | GUI on scdatatools. Public README describes pre-alpha status. Head and tree were pinned; full source audit was limited by GitLab retrieval. |
| [wwiser](https://github.com/bnnm/wwiser/tree/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3) | `6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3`, 2026-11-04 | Adjacent Wwise BNK parser/TXTP generator; no P4K reader. No explicit licence file was found in the owning tree. Facts only. |
| [vgmstream](https://github.com/vgmstream/vgmstream/tree/6802153425adde134e49394fc653455166d7d2f2) | `6802153425adde134e49394fc653455166d7d2f2`, 2026-11-05 | Adjacent game-audio decoder/TXTP player; no Star Citizen Archive reader. `COPYING` grants use/copy/modification/distribution subject to notices; bundled codec dependencies have their own licences. |

The [scdatatools 1.0.4 publisher release](https://pypi.org/project/scdatatools/1.0.4/)
was uploaded 2022-08-02. Its source tarball SHA-256 is
`917289ca636c46bbb48761b379cbe76cef8a3068701d357ba853bcedf85ffa46`, checked before reading.
The public documentation identifies itself as 1.0.3, with a 2022 changelog; a recently crawled
page is not a recent tool release.

Two smaller discovery controls were pinned but not promoted into additional deep comparisons:
[unp4k_rs `ad0a275afaa8e67d517d2201acaf595a491ebabf`](https://github.com/StarCitizenToolBox/unp4k_rs/tree/ad0a275afaa8e67d517d2201acaf595a491ebabf)
is archived and its README recommends StarBreaker; its licence is GPL-3.0.
[scorg-tools/Blender-Tools `0a318aae6870ebed0c1e19dd100c4aaf230b1bde`](https://github.com/scorg-tools/Blender-Tools/tree/0a318aae6870ebed0c1e19dd100c4aaf230b1bde)
is Apache-2.0 and documents a StarFab companion workflow for loadouts/materials/paints and
GUID import. That workflow does not itself establish a new P4KX format gap.

## Capability comparison

“Generic” means access to source database Records, not a typed gameplay catalog matching P4KX.
“External” means another program/library performs the conversion. “Not present” refers to the
inspected tree/scope, not an assertion about every private or future build.

| Tool | Archive | DataForge/catalog | Models/materials | Textures | Animations | Audio |
| --- | --- | --- | --- | --- | --- | --- |
| Svarog | v1/v2 P4K, nested containers, detailed layout/install metadata | Accepts versions 5–8, generic query/XML/schema and C-header output; no P4KX-style typed catalog | No model converter in workspace | Split DDS merge | No animation decoder in workspace | No Wwise decoder in workspace |
| scdatatools 1.0.4 / historical StarFab | ZIP-derived P4K reader, nested containers, CryXML conversion | Generic DataCore; blueprint processor extracts selected Entity relationships | Model conversion delegates to cgf-converter; Blender node/material and blueprint assembly layer | DDS/alpha sidecar merge; external image converters | Declaration/dependency collection is not a native CAF/DBA key decoder | BNK extraction/event traversal plus external conversion; some HIRC traversal is heuristic |
| Cryengine-Converter | Consumes extracted assets; no direct P4K reader demonstrated | No DataForge/catalog subsystem | CrCh/IVO geometry and skeletons to USD/glTF/GLB/Collada; MTL and shader `.ext` declarations | DDS merging/conversion pipeline | Native CAF/DBA and legacy controller readers; numerical disagreements need verification | Not present |
| CryBlend 0.1.6 | `.p4k` streaming explicitly unimplemented; filesystem/ordinary ZIP backends | Not present | Native Blender geometry/rig/CDF/material import; partial compiled morphs and physics helpers | DDS header diagnostics; Blender loads image pixels | Native CAF/DBA-to-Blender Actions; pose-layer restrictions and root-motion omissions | Not present |
| wwiser | No P4K reader | Not present | Not present | Not present | Not present | Versioned BNK field parser, names and TXTP graphs; incomplete Wwise playback simulation |
| vgmstream | No P4K reader | Not present | Not present | Not present | Not present | WEM codec/channel/loop interpretation and TXTP playback; adjacent validation reference |

Owning source anchors for this matrix:
[Svarog workspace](https://github.com/19h/svarog/blob/2e46eff4f7d475b8826ccfdc01c4a95db764db0d/Cargo.toml),
[DataCore version handling](https://github.com/19h/svarog/blob/2e46eff4f7d475b8826ccfdc01c4a95db764db0d/crates/svarog-datacore/src/database.rs),
[DDS merge](https://github.com/19h/svarog/blob/2e46eff4f7d475b8826ccfdc01c4a95db764db0d/crates/svarog-dds/src/merge.rs),
[Converter output scope](https://github.com/Markemp/Cryengine-Converter/blob/7b951dbb254ab99f560e8ffbf64a771c70ccef74/README.md),
[CryBlend limitations](https://github.com/Tzeentchnet/CryBlend/blob/48b9b6c4a4f4c4dded1f2118eb172a5ff7a7eccc/README.md#known-issues--limitations),
[CryBlend DDS diagnostics](https://github.com/Tzeentchnet/CryBlend/blob/48b9b6c4a4f4c4dded1f2118eb172a5ff7a7eccc/cryengine_importer/io/dds.py),
[wwiser parser](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py),
[vgmstream Wwise reader](https://github.com/vgmstream/vgmstream/blob/6802153425adde134e49394fc653455166d7d2f2/src/meta/wwise.c).
The scdatatools release source seams are `engine/chunkfile/converter.py`,
`engine/textures/dds.py`, `sc/blueprints/base.py`, `sc/blueprints/processors/p4k/xml.py` and
`wwise/bnk.py`; use the pinned publisher tarball rather than assuming those paths are identical
at a later GitLab head.

## Reproduced v2 reader gaps

### Extension directory and name table

Svarog's v2 reader sums the base and extension entry counts, CDR sizes and name-table sizes,
validates the combined count against `204 * count`, and iterates all resulting records. Its
[packed trailer definition](https://github.com/19h/svarog/blob/2e46eff4f7d475b8826ccfdc01c4a95db764db0d/crates/svarog-p4k/src/zip/eocd_v2.rs)
and [reader](https://github.com/19h/svarog/blob/2e46eff4f7d475b8826ccfdc01c4a95db764db0d/crates/svarog-p4k/src/archive.rs#L1606)
provide independently inspectable format evidence. No Svarog runtime result is claimed.

P4KX's [directory_v2.rs](../crates/p4kx-archive/src/directory_v2.rs) skips the extension count
and CDR-size fields, reads only the base name-table size and loops only over the base CDR.
An otherwise valid v2 Archive can therefore list and extract successfully while omitting its
extension Entries without an issue.

A fresh private probe used `p4kx-archive` from the pinned checkout, built `--release --offline`.
Four independently encoded 69,632-byte Archives contain only two synthetic `abc` payloads.
They have aligned payloads, an install block, CDR at 65,536, two 204-byte records, terminated
names and an aligned EOF. A separate PowerShell layout decoder checked combined fields,
name/payload ranges, install progress, alignment and marker; the first three fixtures have no
structural defects. Every Entry returned by P4KX was consumed through
`Archive::open_entry_verified` and its CRC-32C passed.

| Synthetic input | Header/source facts | P4KX result |
| --- | --- | --- |
| Base control | Base count 2, extension 0; base CDR 408 bytes | 2 Entries, both `abc`; zero issues |
| Extension | Same payload/CDR/name bytes; trailer changes to counts 1+1, sizes 204+204 and split name sizes | Only first Entry; zero issues |
| Incomplete | Base control with status bit 0 set | 2 Entries; zero issues |
| Malformed extension size | Counts 1+1 but CDR sizes 204+0 | Only first Entry; zero issues despite combined size/count inconsistency |

The extension result isolates the ignored trailer fields: it changes no actual payload, CDR
record or name bytes relative to the successful control. The malformed case proves that an
ignored extension can also conceal a structural mismatch.

The extension fixture SHA-256 is
`a2345da5fe2d5fb7edfa0701eafd6df97fac8a0c8dabd8252fc216e573b909f2`;
the base control is `094ca80c9c008829589bc4cb34271d905a078fd7574b9703af76a9f2f6e51499`.
These hashes identify synthetic fixtures only.

**Implementation issue: [#1106 — read and validate v2 extension CDR/name sections](https://github.com/p4kx-org/p4kx/issues/1106).** This is a reader bug
in an explicitly supported layout, not a request for Archive writing. Implementation seams:
`directory_v2.rs::Trailer`, `read_trailer`, `read_entries`, `read_range`, `layout::check_entry_count`
and `tests/it/layout_v2.rs`. Acceptance criteria:

- Add counts/sizes with checked arithmetic, validate combined record size and bounds, and count
  extension Entries in top-level/container limits before allocating.
- Read the full name table and retain base-before-extension directory order; an extension name
  can begin outside the base name-table slice.
- The two-entry extension fixture lists/reads both payloads through the public API, including
  verified reads; the ordinary all-base control remains identical.
- Reject inconsistent extension count/size, truncated extension records/names and arithmetic
  overflow with precise Archive errors. Apply container limits to the combined count.
- Preserve the existing paths, checksum/encryption handling and per-Entry issue semantics.

### Incomplete status and install progress

The v2 trailer includes a status byte at offset `0x68`; Svarog explicitly treats bit 0 as an
incomplete Archive and refuses it. It also reads each Entry's install-progress value and checks
that it does not exceed the compressed size. P4KX stops decoding trailer metadata after the
base name-table size and exposes no incomplete status. The synthetic status-bit case above
therefore opens without any warning.

**Implementation issue: [#1107 — expose and enforce Archive installation completeness](https://github.com/p4kx-org/p4kx/issues/1107).** Keep this separate
from extension directory enumeration: a complete directory can still describe an incomplete
installation. Seams: `directory_v2.rs`, `Archive`/`OpenOptions`, Archive-wide issue reporting,
`p4kx info --json` and export source/completeness metadata. Acceptance criteria:

- Preserve the raw status byte and install-progress declarations, distinguish unknown bits
  from the known incomplete bit, and avoid silently treating incomplete input as complete.
- Define an explicit normal-open policy: reject known incomplete input or expose a mandatory
  Archive-wide diagnostic that front-ends/exports propagate. A lenient/recovery mode must
  remain visibly incomplete.
- Synthetic complete/incomplete cases with identical directory/payload bytes differ in the
  reported completeness. Cover short install blocks, progress beyond stored payload size,
  unknown status bits and a zero-entry Archive.
- Do not infer that all payloads are usable merely because a bank/model/sample read succeeds.
  Do not require mutating, repairing or writing the input Archive.

## Additional source metadata families

### Archive layout, signatures and installation metadata

Svarog's [Archive metadata types](https://github.com/19h/svarog/blob/2e46eff4f7d475b8826ccfdc01c4a95db764db0d/crates/svarog-p4k/src/archive.rs#L74)
and [JSON list output](https://github.com/19h/svarog/blob/2e46eff4f7d475b8826ccfdc01c4a95db764db0d/src/main.rs#L1550)
retain source facts that P4KX currently discards: raw DOS modification words, 128-byte Entry
signatures, per-Entry installed-byte progress, sector size, payload/install/CDR/name-table/end
record locations, free ranges and the two stored 32-byte trailer digest slots. P4KX already
retains Entry CRC-32C and stored-payload SHA-256; those are not newly missing features.

**Implementation issue: [#1108 — export Archive source layout/install metadata](https://github.com/p4kx-org/p4kx/issues/1108).** This is a focused Archive
metadata contract, separate from media source metadata in #1092/#1099. Preserve raw integer
words and optional byte fields with provenance and layout version; represent unavailable v1/v2
fields explicitly. Do not label a retained digest/signature “verified” without performing its
verification. Preserve both digest slots without inventing the second slot's meaning.

Acceptance criteria: independent v1/v2 fixtures with non-default timestamps/signatures and
nonempty install/free-range data; exact source-value JSON; checked offsets/ranges; stable source
identity fields; no local key path, key contents or launcher-credential data; schema/CLI/MCP
agreement. A read-only layout report must not depend on Archive writing or game installation
repair. The incomplete-status behaviour above can consume this contract but must have its own
behavioural tests.

### Shader extension declarations

Cryengine-Converter reads `Shaders/<name>.ext` and retains shader Version,
UsesCommonGlobalFlags, and Property information including Name, numeric Mask,
editor Property label, Description, DependencySet/DependencyReset and Hidden. See its
[parser](https://github.com/Markemp/Cryengine-Converter/blob/7b951dbb254ab99f560e8ffbf64a771c70ccef74/CgfConverter/Parsers/ShaderExtParser.cs),
[property model](https://github.com/Markemp/Cryengine-Converter/blob/7b951dbb254ab99f560e8ffbf64a771c70ccef74/CgfConverter/Models/Shaders/ShaderProperty.cs)
and [USD loader](https://github.com/Markemp/Cryengine-Converter/blob/7b951dbb254ab99f560e8ffbf64a771c70ccef74/CgfConverter/Renderers/USD/UsdRenderer.cs#L45).
Its parser's dictionary does not preserve duplicate names, and its regex is not evidence that
all nested/unknown syntax is handled correctly.

Crytek's [shader-development documentation](https://www.cryengine.com/docs/static/engines/cryengine-5/categories/23756813/pages/23306638)
independently describes `.cfx` effect files, `.cfi` includes and same-name `.ext` material-flag
definitions. The declarations supply names and bit masks; they are distinct from each MTL's
selected flags. Runtime declarations also exist in `RunTime.ext`. This establishes the format
family, not the exact current Star Citizen shader implementation.

P4KX [MTL parsing](../crates/p4kx-model/src/mtl_parse.rs) retains StringGenMask and unknown MTL
attributes, and the Blender material package contains selected flag names. No shader-extension
reader/export was found. Raw MTL export #1097 should therefore link to, rather than silently
stand in for, a shader declaration source family.

The independent installed-PTU probe found the source family inside nested `.pak` containers:
one of three inspected containers contains **62 `.ext` files**; the largest is 26,855 bytes.
All are comment-led non-XML text. The other two containers contain `.cfib`, `.cfxb` and `.shcb`
compiled data, inventoried without a semantic-decoding claim. A top-level-only `.ext` search
therefore misses the confirmed source family.

A sanitized bounded structural check of two `.ext` sources found three and six braced
Property blocks. Both use parenthesized Version, Property labels and Description, with equals
forms for Name and hexadecimal Mask. The second also uses DependencySet/DependencyReset
symbol declarations and a bare Hidden marker. No source values, source comments or private
asset identities are reproduced here. UsesCommonGlobalFlags is a supported-format field from
the primary reference parser, not a claim that these two samples contain it; the two-file
check does not establish the complete grammar of all 62 definitions.

**Implementation issue: [#1109 — export shader declarations and material-to-definition links](https://github.com/p4kx-org/p4kx/issues/1109).**
Add a `shader` provider to #1089 with bounded lexical/structural parsing and nested-container
provenance. Contract: source path and complete container chain, source bytes/content identity,
shader/name/version, ordered properties, raw numeric masks,
dependency expressions, runtime/common flag scope and unknown directives/blocks. Link an MTL
shader/flag to declarations with explicit missing/ambiguous/unresolved states. Keep duplicate
names and source order; do not silently turn malformed masks into zero.

Tests/acceptance: independent synthetic definitions using equals/parenthesis forms, comments,
quoted values, unknown/nested syntax, repeated properties, a mask above `2^53`, absent files and
case-varying Archive paths; deterministic structured export plus retained source; declaration
counts reconcile with source. Do not execute/compile shaders and do not claim renderer parity
from descriptions or infer that a texture slot is always an ordinary image.

The Converter USD rules still start from `material.StringGenMask`; they return defaults when it
is empty. Its numeric property Mask parser does **not** establish superior rendering of an MTL
that has only numeric GenMask. Its own development notes discuss MechWarrior shader work;
Star Citizen applicability must be checked against installed declarations.

## Animation and media disagreements resolved by independent checks

| Seam | Source evidence | Decision |
| --- | --- | --- |
| CAF `AnimInfo` version `0x901` | [Converter](https://github.com/Markemp/Cryengine-Converter/blob/7b951dbb254ab99f560e8ffbf64a771c70ccef74/CgfConverter/CryEngineCore/Chunks/ChunkIvoAnimInfo_901.cs) and [CryBlend](https://github.com/Tzeentchnet/CryBlend/blob/48b9b6c4a4f4c4dded1f2118eb172a5ff7a7eccc/cryengine_importer/core/chunks/ivo_anim_info.py) read end frame at byte 12 and quaternion at 16; P4KX reads end frame at 16 through the record helper shared with DBA. | The independent PTU probe resolves this for P4KX: word at 16 equals the maximum decoded key frame for all 54,391 CAFs, versus only 562 at word 12; quaternion at 20 is unit length for all 54,391, versus 1,321 at 16. The inspected Converter/CryBlend layout is wrong for these current files. |
| Time format low byte `0x42` | [Converter helper](https://github.com/Markemp/Cryengine-Converter/blob/7b951dbb254ab99f560e8ffbf64a771c70ccef74/CgfConverter/Models/Structs/IvoAnimationStructs.cs) linearly interpolates start/end for time nibble 2; P4KX decodes explicit bitmap-set frames. | [Independent StarBreaker source](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/animation/codec.rs) agrees with the P4KX bitmap layout. Preserve nonuniform frame maps; the Converter interpolation assumption is not a valid current-layout oracle. |
| Quantised translations `0xC2` | [Converter CAF](https://github.com/Markemp/Cryengine-Converter/blob/7b951dbb254ab99f560e8ffbf64a771c70ccef74/CgfConverter/CryEngineCore/Chunks/ChunkIvoCAF_900.cs) uses active-axis interleaving; P4KX decodes planar columns. | [Independent StarBreaker source](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/animation/codec.rs) agrees with P4KX axis-major columns for `0xC2`. Do not substitute the Converter interleaving assumption. |
| Rotation format `0x83` | P4KX explicitly decodes SmallTree64; the Converter helper only selects SmallTree48 for `0x82`, otherwise reads raw float quaternions. | Wider advertised animation support is not proof of complete key-format handling. |
| CrCh compiled morphs | [CryBlend decoder](https://github.com/Tzeentchnet/CryBlend/blob/48b9b6c4a4f4c4dded1f2118eb172a5ff7a7eccc/cryengine_importer/core/chunks/morph_targets.py) and [shape-key bridge](https://github.com/Tzeentchnet/CryBlend/blob/48b9b6c4a4f4c4dded1f2118eb172a5ff7a7eccc/cryengine_importer/blender/scene_builder.py#L305) consume `CompiledMorphTargets` `0x800/0x802` as absolute deformed positions. `0x801` and legacy mesh morph/name tables remain unsupported. | Prior full installed PTU/LIVE census has no CrCh `0x2002`; do not create a PTU regression issue. P4KX's existing DNA facial morph pipeline is a different source family. Raw payload preservation belongs to #1092. |
| Tangents/physics | CryBlend explicitly omits IVO 8-byte tangent-frame decoding and IVO physical-bone types; its rope/primitive helpers are partial preservation. | P4KX already decodes tangent frames. Do not file a generic physics/morph export duplicate of #1092. |
| DDS image output | CryBlend DDS code reads diagnostics rather than image pixels; Svarog's merge code alone does not establish correct arrays/volume topology. | P4KX's installed multi-layer/slice checks and #1098 remain the better evidence. |

## Audio comparison and existing issue mapping

wwiser's [versioned parser](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py)
provides an independent field inventory for Wwise node parameters, effects, state chunks,
RTPC curves, attenuation, modulators and music clip automation. Its generator keeps separate
[game-variable](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/generator/registry/wgamevars.py)
and [transition](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/generator/registry/wtransitions.py)
registries. These facts strengthen #1099/#1100/#1101; they do not justify three new copies of
those tickets. Use version-154-specific fixtures and compare field values, not just successful
bank loads. ATL remains a Star Citizen layer and is already #1102.

wwiser's own [documented missing features](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/doc/WWISER.md#missing-features)
include incomplete pre/post-transition overlaps, real-time switch changes, multi-loop handling,
effects and plugins. A generated TXTP is therefore a selected playback scenario, not an exact
Wwise engine render. P4KX should retain its source/derived distinction.

vgmstream's [Wwise decoder](https://github.com/vgmstream/vgmstream/blob/6802153425adde134e49394fc653455166d7d2f2/src/meta/wwise.c)
separates codec, channel configuration, sample/loop counts and RIFF chunk locations; it handles
more Wwise codec variants than observed in the installed PTU. That makes it a useful optional
codec/sample/channel cross-check, not evidence that PTU contains Opus, ADPCM, XMA or another
unobserved encoding. Its [TXTP documentation](https://github.com/vgmstream/vgmstream/blob/6802153425adde134e49394fc653455166d7d2f2/doc/TXTP.md)
is the playback-format contract for exported scripts.

## Leads not promoted to current compatibility issues

- Svarog supports Archive methods 93 (ZIP zstd) and 101 (zlib-wrapped DEFLATE); P4KX maps only
  0/8/100. The main-audit inventory found only Store and zstd Entries in current PTU: methods
  93/101 are not an installed compatibility need. An additional writer's synthetic output alone
  does not prove a Star Citizen installation requires it.
- Converter/CryBlend also decode `pbxml\0`, a binary XML format distinct from CryXmlB.
  [Decoder source](https://github.com/Tzeentchnet/CryBlend/blob/48b9b6c4a4f4c4dded1f2118eb172a5ff7a7eccc/cryengine_importer/io/cry_xml.py)
  establishes the distinction; no installed PTU need was established here.
- Converter's `.cal` list parser calls the format ArcheAge-specific. Do not turn support for
  other CryEngine games into an unsupported Star Citizen compatibility requirement.
- USD export, editing/repacking Archives, DataCore writing, C-header output, virtual mounts and
  Blender convenience workflows are product-scope choices, not extraction-fidelity defects.
- Historical scdatatools BNK traversal includes byte-pattern ID searching for several complex
  HIRC families. P4KX's versioned object parsers should not regress to heuristic ID scanning.

## Reproduction boundary

The private audit task directory contained these reproducibility files:
`make-fixtures.ps1`, `check-fixtures.ps1`, `fixture-layout.json`, the tiny P4KX reader source and
`result.log`. The generated Archives contain only synthetic bytes. Compiler output uses the
shared audit `build` directory, with TMP/TEMP under the same authorized task root. The scratch
folder is removed when the parent audit completes; durable regression fixtures belong in the
implementation issues/tests, not extracted game content in this repository.

No implementation fix, commit, public issue or third-party binary execution was performed by
this sub-audit. Source evidence, reproduced reader gaps and conditional leads remain separate.
