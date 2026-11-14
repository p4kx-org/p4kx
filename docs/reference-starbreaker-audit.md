# StarBreaker source comparison for v0.5.0

Research date: 2026-11-06. P4KX baseline: [1ee9c0edbfc34e616d2f140951e4eb5bc1c67558](https://github.com/p4kx-org/p4kx/tree/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558).
StarBreaker source baseline: [7d3f01e76307354bbc56a64a70eb5167b1189b64](https://github.com/diogotr7/StarBreaker/tree/7d3f01e76307354bbc56a64a70eb5167b1189b64), resolved from the official default `main` branch on the research date.

This combines a pinned source audit with the bounded official-release and current-P4KX probes documented below. No StarBreaker source build or in-game comparison was performed. The separate [media audit](ptu-media-compatibility-audit.md) and [DataForge/catalog audit](ptu-export-compatibility-audit.md) retain their wider coverage. No assets, private asset names, gameplay values, credentials or keys are included.

## Revision and distribution facts

The latest official release queried on the research date is [v0.3.2](https://github.com/diogotr7/StarBreaker/releases/tag/v0.3.2), published 2026-05-19, targeting `450fa284d4b1cbed6832675297dfb02579c0abd3`. The Windows CLI asset is `starbreaker-cli-v0.3.2-windows-x86_64.zip`; the GitHub release metadata gives SHA-256 `fafd65ca002b9c3bb88ce32199b2345affcdb358655c68c41d0e93574a5d8a3d`. The October main checkout still declares workspace version `0.3.2`. A binary reporting that version therefore does not establish that it implements the October source. Record release and commit independently. Sources: [release](https://github.com/diogotr7/StarBreaker/releases/tag/v0.3.2), [workspace manifest](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/Cargo.toml).

The current source has an MIT license, copyright 2026 Diogo Trindade. P4KX already describes StarBreaker as MIT in ADR-0001. This audit records behavior and structures without copying implementation code. Sources: [StarBreaker LICENSE](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/LICENSE), [P4KX ADR-0001](adr/0001-own-core-instead-of-starbreaker.md).

## Coverage and comparison boundaries

| Area | Current StarBreaker source | P4KX comparison and action |
| --- | --- | --- |
| Archive | Stored, Deflate and Zstandard entry reads; encrypted-Zstandard branch; directory/search APIs. | Useful independent raw-byte comparison over shared supported states. Do not infer checksum or malformed-input correctness from matching extraction alone. |
| DataForge | Generic schema-driven JSON/XML/unp4k XML; `--all` exports non-main Records with unique names. | Existing reference harness already compares generic values. Extend execution coverage to all Records; StarBreaker has no competing domain catalog demonstrated by these commands. |
| Models | Mesh streams, skin/morph data, NMC hierarchy, GLB and decomposed packages. | Compare structural invariants before rendered appearance. Existing #1092 owns source metadata/raw-chunk export; no duplicate issue is needed. |
| Materials | Root material-set data as well as submaterials; source engine-default material resolution. | Root-level metadata is lost before current P4KX exporters. Extend #1097. Engine-default lookup is a separate source difference requiring an installed-source check before an appearance claim. |
| Textures | Raw DDS headers, split main/alpha mips, per-alpha-mip encoding/layout, BC6H float conversion. | #1098 already covers source metadata. The inspected decoder API is not a general array/volume/float oracle. |
| Animations | CAF/DBA track serialization, duplicate hash variants, controller format words, source rig annotations and binding reports. | Extend #1093 with packed controller words and exact clip metadata. Keep source conventions explicit and unconfirmed composition out of scope. |
| Audio | Typed HIRC readers for all numbered v128+ type families, node/property/RTPC/routing structures. | Existing #1099-#1102 cover much of the gap. Additional typed bus/attenuation/effect-family decoders are outside their current raw-retention/shared-node scope. |

Primary implementation sources for this table: [archive.rs](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-p4k/src/archive.rs), [dcb.rs](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/cli/src/dcb.rs), [mesh types](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/types.rs), [decomposed contract](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/docs/decomposed-export-contract.md), [DDS](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-dds/src/dds_file.rs), [animation types](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/animation/mod.rs), [HIRC dispatch](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/mod.rs).

## Confirmed source omission: multi-material root data

`p4kx-model/src/mtl_parse.rs::library` keeps the root's `DefaultPalette` and the existence of `SubMaterials`, then parses its children as materials. For a multi-material, no field of `MaterialLibrary` retains the remaining root attributes, root `PublicParams`, or other root child blocks. Per-submaterial raw retention does not recover those root fields. This is a parser/data-model omission, so merely expanding a JSON DTO cannot fix it. Sources: [P4KX parser](https://github.com/p4kx-org/p4kx/blob/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558/crates/p4kx-model/src/mtl_parse.rs#L29), [MaterialLibrary](https://github.com/p4kx-org/p4kx/blob/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558/crates/p4kx-model/src/mtl.rs#L252).

StarBreaker `parse_mtl` builds `MaterialSetAuthoredData` from the root attributes, every root `PublicParams`, and remaining root blocks for a multi-material. Its decomposed sidecar emits `authored_material_set.attributes`, `.public_params` and `.child_blocks`. These are directly comparable source facts, independent of shader interpretation. Sources: [StarBreaker parse_mtl](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/mtl.rs#L709), [sidecar writer](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/decomposed.rs#L2858).

Update [#1097](https://github.com/p4kx-org/p4kx/issues/1097): retain an ordered root-material-set view, with bounded recursive unknown blocks, and project it through the same material DTO to bulk media output, model info and Blender packages. Preserve the distinction between a root set and a single material rather than merging root shader parameters into every submaterial. Add text/CryXml synthetic fixtures containing nonzero root attributes, root `PublicParams`, and an unknown nested root block. Assert all fields survive reader-to-export, including when no model references the library. An ignored census can report populated root-field families without publishing game contents.

## Source difference: resolving the engine-default material

Both tools correctly map a single-material library to every requested slot and preserve geometry for out-of-range multi-material slots. StarBreaker's `resolve_material_slots` calls `load_default_material`, which loads `Data/Materials/material_default.mtl` through the Archive and appends the parsed material once when required. P4KX `MaterialPlan::new` instead always constructs `Material::engine_default`, whose surface inputs are fixed host values; it does not load the default library or its texture/source provenance. Sources: [StarBreaker material_slots.rs](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/material_slots.rs), [P4KX material plan](https://github.com/p4kx-org/p4kx/blob/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558/crates/p4kx-gltf/src/materials.rs#L40), [P4KX engine_default](https://github.com/p4kx-org/p4kx/blob/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558/crates/p4kx-model/src/mtl.rs#L170).

This establishes a source-resolution/provenance difference, not the rendered correctness of either tool. Before a defect claim, verify that the installed default library exists and compare its populated surface/texture fields with the fallback, plus census the models that actually use missing slots. A clean implementation would resolve the source default at the service/companion boundary and give all consumers a shared material disposition, retain raw source-slot identity, and distinguish an unavailable source default from a successfully loaded one. It should not make the format crate depend on Archive or silently borrow submaterial zero. A synthetic default library with nontrivial surface inputs is sufficient to test the seam.

A tracker search for `material_default` found no existing issue on the research date. The related #1091 concerns safe source-path normalization, not fallback-material selection; #1097 concerns source metadata, not rendering resolution.

## Additional animation facts for #1093

StarBreaker's `BoneChannel` retains both 16-bit packed controller words (`rot_format_flags`, `pos_format_flags`). In P4KX, `Side::read` reads the corresponding word, separates its time and key encoding bytes for decoding, then the public track loses the original word. P4KX `read_record` also retains only the interpreted additive bit, frame rate and end frame from the fixed-size clip record. Source metadata retained by StarBreaker includes start rotation and start position components. Sources: [StarBreaker channel fields](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/animation/mod.rs#L94), [P4KX record and Side](https://github.com/p4kx-org/p4kx/blob/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558/crates/p4kx-model/src/animation_read.rs#L25), [StarBreaker DBA metadata](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/animation/dba.rs#L146).

Extend [#1093](https://github.com/p4kx-org/p4kx/issues/1093) with both packed words, source record index/span and an exact retained record representation. Any typed projection of the remaining clip metadata must document the validated version/layout and preserve unknown bits. Do not adopt StarBreaker's claim that a missing position component is implicitly zero without independent evidence: StarBreaker starts its 48-byte DBA entry scan after a four-byte count, while P4KX starts records after its eight-byte metadata header; their member offsets differ even where the absolute frame-rate/end-frame offsets align. Source anchoring and additive composition remain #668's separate question. Source: [P4KX DBA layout](https://github.com/p4kx-org/p4kx/blob/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558/crates/p4kx-model/src/animation_dba.rs#L179).

StarBreaker `clip_to_json` writes frame-key arrays and retains repeated controller hashes as multiple channel variants, then annotates source skeleton path/name when it can resolve them. `binding_report` compares hash-only channels with source rig/meshsetup/chrparams and GLB names. This supports #1093's existing requirement that unbound and duplicate tracks survive export. Keep reports supplementary; do not convert an unbound track into a guessed joint. Sources: [serialization](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/animation/serialise.rs), [binding report](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/animation/binding_report.rs).

## Focused animation decoder cross-check

The current StarBreaker CAF `parse_anim_info` reads frame rate at byte offset 4 and does not read an end-frame field. Its JSON frame count comes from the latest decoded key. It cannot independently decide whether a P4KX CAF header duration field belongs at byte 12 or 16; that question requires direct source-byte/key-limit or engine-format evidence. Sources: [CAF info reader](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/animation/caf.rs#L26), [JSON frame count](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/animation/serialise.rs#L77).

For `0xC2` translations, both current readers use contiguous arrays per active axis, with inactive axes taking their offsets. For bitmap times, StarBreaker masks the packed word's low nibble and its format 2 reads start/end u16 followed by LSB-first bits; P4KX format `0x42` uses that same structural layout. StarBreaker includes synthetic multi-axis and irregular-bitmap tests. These source checks establish agreement on those two shapes, not correctness of every decoded key. Sources: [StarBreaker codecs](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/animation/codec.rs), [codec tests](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/animation/bake_tests.rs#L338), [P4KX animation_keys.rs](https://github.com/p4kx-org/p4kx/blob/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558/crates/p4kx-model/src/animation_keys.rs).

A reference run must distinguish recovered keys from fallback-generated keys: StarBreaker uses uniformly spaced times when bitmap population disagrees with the key count, linear times for unknown time codes, a default quaternion decoder for unknown rotation codes, and zero positions for unknown position codes. P4KX returns explicit errors for unknown encodings or inconsistent bitmap counts. Do not broaden P4KX's comparator allow-list to excuse arbitrary invented timing. The packed-position active-axis sentinel and small-tree quaternion scaling also differ slightly between the implementations; synthetic boundary cases and format-grounded float tolerances are needed before classifying numerical differences. Source: [StarBreaker codec fallbacks](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-3d/src/animation/codec.rs), [P4KX validation](https://github.com/p4kx-org/p4kx/blob/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558/crates/p4kx-model/src/animation_keys.rs).
## Further typed Wwise data beyond the existing issues

P4KX `HircValue` decodes Sound, Action, Event, random/sequence/switch/blend containers and four music families. Its other recognized kinds remain `Raw`. StarBreaker has source readers for State, ActorMixer, bus/aux-bus, attenuation, effects, modulators, dialogue and device objects. These readers identify feasible additional source projections; their presence does not establish exact interpretation on a different Wwise version. Sources: [P4KX dispatch](https://github.com/p4kx-org/p4kx/blob/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558/crates/p4kx-audio/src/hirc.rs#L198), [StarBreaker dispatch](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/mod.rs#L290).

[#1099](https://github.com/p4kx-org/p4kx/issues/1099) deliberately guarantees recoverable raw payloads for these families. [#1100](https://github.com/p4kx-org/p4kx/issues/1100) covers shared node parameters but explicitly excludes complete bus/effect payload decoding. [#1101](https://github.com/p4kx-org/p4kx/issues/1101) owns container/music decision structures. Extend #1100 with State property objects and ActorMixer node/children where appropriate, and treat DialogueEvent's decision tree as #1101 work rather than creating overlapping trees.

The following implementation issues follow #1099 and #1100 and are tracked under the media overview #1088, with the reference-tool audit under [#1103](https://github.com/p4kx-org/p4kx/issues/1103):

| Tracked unit | Concrete source fields | Primary reference |
| --- | --- | --- |
| [#1110: bus and aux-bus objects](https://github.com/p4kx-org/p4kx/issues/1110) | Original output/override bus and device share-set IDs; properties, positioning/aux sends, recovery time, maximum duck volume; ordered duck bus/volume/fade/curve/target-property entries; effect and metadata slots; RTPC/state data. | [AudioBus::parse](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/bus.rs#L24) |
| [#1111: attenuation objects](https://github.com/p4kx-org/p4kx/issues/1111) | Cone enable and inside/outside angles, outside attenuation/low-pass/high-pass values; all signed curve-use indices; ordered curve scaling/points/interpolation; RTPC bindings. | [Attenuation::parse](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/attenuation.rs) |
| [#1112: effects, modulators and devices](https://github.com/p4kx-org/p4kx/issues/1112) | Effect/plugin identifier, exact opaque plugin parameter bytes and bank-data references; RTPC/state overrides; LFO/envelope/time property/ranged bundles; device effect-slot and bypass metadata. | [FxBase](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/bus.rs#L139), [Modulator](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/modulator.rs), [AudioDevice](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/misc.rs#L87) |

Each enhancement should use one shared node/property/RTPC implementation, retain raw payloads and original integer codes, and expose typed fields additively through ObjectView/CLI/MCP and the media provider. A plugin ID does not prove knowledge of its opaque parameter format. Do not cast every property word to float, normalize signed curve sentinels away, deduplicate source entries by short ID, evaluate playback, or label unknown units as established.

Agent acceptance criteria for each unit: synthetic nonzero fixtures for every branch; explicit supported-version gating (start with proved v154 layouts); bounded counts and truncated/overflow cases; exact consumed spans with deliberate trailing-byte disposition; duplicate IDs scoped by bank/language/source index; original unknown/nonfinite bit preservation; stable malformed-versus-unsupported diagnostics; raw round-trip and old resolver/output compatibility. An ignored installed-Archive census should reconcile typed/raw/malformed counts and compare selected independent fields without publishing source data. Current source proves the opportunity, not successful installed PTU parsing of these additional families.

## Executed installed-PTU checks

A private Rust probe used the current checkout's Archive, CryXml, model, extraction, assembly and testkit crates. It was built with `--release --offline`, and the final runtime reads used the configured key helper without logging it. Official StarBreaker v0.3.2 was used separately for the bounded media outputs. All code, logs and outputs stayed in the audit task directory outside the repository; game files were read only. The shader containers were opened in place through Archive, without dumping their multi-gigabyte contents.

| Structural check | Observed result |
| --- | --- |
| Top-level Archive | 1,407,271 Entries; 163,480,088,576 bytes |
| Compression/encryption | 131,876 stored/plain; 1,271,051 Zstandard/plain; 4,344 Zstandard/encrypted; no other directory states |
| End signature | Last four bytes are zero; not the `Jiji` marker |
| Small raw media references | 22 CRC-verified Entries: two each for CGF/CGFM/CGA/skin/skinm/CHR/DDS/DDS alpha/WEM/BNK/socpak, selected per observed extension/compression/encryption bucket; each positive-size and at most 4 MiB |
| Material readers | All 26,307 `.mtl` libraries parsed; 19,409 are multi-materials |
| Lost multi-material root facts | All 19,409 roots carry `MtlFlags`; 19,392 carry each of `MatTemplate`, `MatSubTemplate` and `vertModifType`. 19,389 roots have `PublicParams` attributes, spanning 50 distinct parameter names; nine have `VertexDeform` root blocks. These source-root fields have no current MaterialLibrary field. |
| MatRef query | No `MatRef` elements, owning libraries or `File` attributes occurred across those 26,307 libraries; do not promote a current-PTU MatRef defect from the external implementation alone |
| Source default material | Present as one single-material library, with one texture slot, one public parameter and three additional attributes. Shader, surface type, diffuse/specular values, textures, parameters and additional attributes differ from the host fallback. No actual engine-fallback selection is proved by this observation. |

The root-material result independently confirms that [#1097](https://github.com/p4kx-org/p4kx/issues/1097) must change parsing/data modeling as well as the writer; that issue was updated accordingly. [#1093](https://github.com/p4kx-org/p4kx/issues/1093) was updated with exact header/controller retention and version-specific validation. State/ActorMixer decoding remains in the updated #1100, and DialogueEvent conditions in the updated #1101. No shader interpretation is required to preserve the root facts.

### Nested shader inventory

There are no top-level `.ext`, `.cfx`, `.cfi`, `.shader` or `.shaderext` Entries. Three shader-related PAK containers contain the relevant families:

| Container size | Entries | Extension inventory |
| --- | --- | --- |
| 2,736,982,671 bytes | 171,490 | 261 CFIB, 156 CFXB, 171,073 SHCB |
| 2,060,902,999 bytes | 220,333 | 261 CFIB, 156 CFXB, 219,916 SHCB |
| 41,164 bytes | 62 | 62 EXT |

All 62 EXT files were read with verified Entry reads. Each begins with the same four technical signature bytes `2f 2f 20 28`, is non-XML text, and the largest is 26,855 bytes. Thus an XML-only export/parser would miss the actual declaration grammar. Two bounded declarations were passed privately to the parallel shader audit for grammar-only inspection; its report and [#1109](https://github.com/p4kx-org/p4kx/issues/1109) own those grammar/export findings. The compiled CFIB/CFXB/SHCB families were inventoried by directory, not semantically decoded. A shader source inventory must include nested-container provenance and preserve compiled payloads as opaque; it must not claim these byte families are parsed shaders.

### Bounded entity and texture comparison

The official release accepted `entity export` with LOD 2, texture mip 4, two worker threads, a 1,536 MiB allocation cap, and attachments/interiors/lights disabled. P4KX built the same Entity's root part with matching LOD/mip/material settings and a one-part assembly budget. That budget deliberately reports one truncation/limit issue for the omitted Loadout; this is a root-only comparison, not complete assembled-ship parity.

| Root GLB artifact | Current P4KX | StarBreaker release v0.3.2 |
| --- | --- | --- |
| File bytes | 6,460,768 | 8,642,472 |
| Nodes | 274 | 274 |
| Meshes / primitives | 84 / 282 | 87 / 337 |
| Triangles | 69,036 | 74,136 |
| Materials / images | 21 / 41 | 24 / 54 |
| Skins / animations | 0 / 0 | 0 / 0 |
| P4KX repository artifact validator | Pass | Stops at an unreferenced texture |

P4KX reports zero texture fallbacks in this case. Both GLBs parse as glTF; StarBreaker's unreferenced texture violates a stricter repository artifact invariant, so this does not establish that core glTF disallows its file. Different geometry/material suppression, packing and shader approximations remain to be analyzed before assigning any difference to a P4KX defect. Byte equality of the GLBs is not expected.

A verified ordinary 256-by-256 BC1 DDS with no alpha tail was decoded at mip zero by both tools. Both emitted full 8-bit RGBA images of the expected extent. Alpha bytes agree exactly; maximum absolute error is one byte value in each RGB channel. There are 5,877 differing red, 4,784 green and 5,049 blue components, each differing by one. This establishes a small ordinary-BC rounding difference; it is not a decoder failure. A separate four-by-four canary agreed byte-for-byte after PNG decode.

Two bounded external-tool limitations were recorded rather than assigned to P4KX: a 768 MiB allocation cap stopped StarBreaker during Archive opening (the established 1,536 MiB cap completed), and its DDS command prepended `Data` to a valid non-Data-root source Entry and could not find it. The representative comparison therefore used a Data-root DDS. These are test-selection/resource constraints, not evidence of broad media parity.
## Reference execution plan and reliability limits

Use distinct directories under the approved task scratch folder for release output, main-source output and P4KX comparison. Pin and hash each executable. The following current-source CLI shape is supported by `cli/src/dcb.rs`:

```text
starbreaker dcb extract --dcb <private raw Game2.dcb> --format json --output <private reference folder> --all
```

The coordinating release run confirmed that official v0.3.2 rejects `--all`; the command above describes pinned current source, not the release binary. Without `--all`, StarBreaker chooses only the main Record of each source filename. With it, non-main output names append sanitized record name and GUID. `--filter` is a glob over the source filename, not a type selector. P4KX's existing `P4KX_REF_STARBREAKER` reference test understands the Record JSON header, embedded same-file Records and the known narrowly scoped weak-pointer/dangling-reference allowances. Run it with the real Archive and key helpers after producing the folder. Sources: [StarBreaker CLI](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/cli/src/dcb.rs), [existing reference test](https://github.com/p4kx-org/p4kx/blob/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558/crates/p4kx-extraction/tests/reference/starbreaker.rs), [existing instructions](differential-verification.md).

Do not use process success alone as completeness evidence. StarBreaker's individual-record exporter prints per-file conversion/write errors and still reaches `Ok(())`; verify output count, scan stderr and compare the expected Record inventory. Similarly, `dds info` can fall back to header-only parsing after a split-merge failure, so an info command does not prove all pixel payloads decoded. Sources: [DCB extract](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/cli/src/dcb.rs#L101), [DDS info](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/cli/src/dds.rs#L119).

For audio, `starbreaker wwise dump <private bank file>` emits hierarchy JSON in current source. Its HIRC dispatch does not receive the bank version and converts any reader error into `Unknown` with raw bytes; it also does not establish complete body consumption. Census typed versus Unknown results and compare exact field values/spans, not merely process exit or object counts. Sources: [dump command](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/cli/src/wwise.rs), [parse dispatch](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/mod.rs#L297).

For texture comparisons, select ordinary supported 2D BC/RGBA inputs first. `DdsFile` stores one byte vector per mip; layout computation multiplies by one/six cube faces but does not enumerate DX10 array elements or volume slices, while `decode_rgba` accepts only a mip index. Its uncompressed DXGI decoder covers a limited byte-format set, and the dedicated float function accepts unsigned BC6H. P4KX's arrays/volumes/integer/other-float tests therefore remain necessary independent coverage. This is a scope limit of the chosen oracle, not evidence of a P4KX regression. Source: [DdsFile layout/decode](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-dds/src/dds_file.rs).

For WEM, recognition of PCM/ADPCM/Opus identifiers in `WemCodec` must not be counted as a completed decoder: the inspected decode module exposes Vorbis-to-OGG reconstruction. Compare decoded sample streams or properly normalized OGG packets for the shared supported codec; compare source RIFF metadata separately. Sources: [codec recognition](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wem/src/fmt.rs), [decode API](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wem/src/decode.rs).

No repository implementation fixes or commits were performed. The coordinating audit published the linked issues and updates. Its final raw-media run compared 34 Entries with unp4k: all 34 byte-identical without allowances (six stored/plain, 26 Zstandard/plain and two encrypted), including animation binary samples. That broader comparison supplements the 22-entry private selection generated by this probe; it does not turn the bounded GLB/DDS comparison into exhaustive semantic media parity. Private probes, extracted samples and outputs are removed after coordination completes.

## StarBreaker pull request #54

Audit date 2026-11-13. The GitHub API reported 32 commits and 123 changed files at
head [`14ae2d99`](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/docs/decomposed-export-contract.md), against the StarBreaker
baseline above. The [pull request](https://github.com/diogotr7/StarBreaker/pull/54)
was open. The upstream runtime claims describe its 4.10.2 LIVE export. This
comparison uses source, the P4KX integration checkout at `39ede42fa`, and the
bounded P4KX checks below.

| Finding | P4KX outcome |
| --- | --- |
| The decomposed writer builds real GLBs and optionally preserves sparse authored material indices. P4KX already produces mesh GLBs and uses a dense `p4kx_index` for Blender assignment. | Preserve both identities with explicit primitive/source mapping under [#1092](https://github.com/p4kx-org/p4kx/issues/1092). The [StarBreaker builder](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/gltf/glb_builder.rs) and [P4KX package writer](../crates/p4kx-blender/src/meshes.rs) use different numbering contracts. |
| The GLB verifier adds structural parsing. P4KX's [import preflight](../blender_addon/p4kx_importer/package.py) admitted a synthetic unsupported GLB version with a wrong total length and missing declared BIN data. | [#1134](https://github.com/p4kx-org/p4kx/issues/1134) owns bounded preflight of the GLB and actual manifest references. [StarBreaker validator](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/validation/glb.rs) |
| The lazy catalogue combines named parameters, DBA-only and loose CAF clips, ordered contributor rigs, physical block indices and Mannequin metadata. Event lookup joins the authored CAF path. | Extend [#1093](https://github.com/p4kx-org/p4kx/issues/1093), [#1094](https://github.com/p4kx-org/p4kx/issues/1094) and [#1095](https://github.com/p4kx-org/p4kx/issues/1095). Keep authored default-pose selectors distinct from an optional bake. Endpoint similarity and unknown-code fallback values are insufficient as authored timing or composition evidence. [Catalogue](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/pipeline/clip_catalogue.rs) · [Default pose](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/pipeline/default_pose.rs) |
| The source decoder separates collision primitives, damage and SDF blobs, reads vehicle grid proxies and uses actual CDF attachments. P4KX identifies `StatObjPhysics` but its generic chunk inventory does not expose these typed shapes. | [#1128](https://github.com/p4kx-org/p4kx/issues/1128) reuses [#1092](https://github.com/p4kx-org/p4kx/issues/1092) for chunk IDs and raw bytes. Confirm format versions, unknown dispositions and frame transforms on the original bytes. [Physics reader](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/ivo/physics.rs) · [Grid joins](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/physics_grid.rs) |
| Container-local room registry GUIDs and AreaComponent segments carry geometry beyond model boxes. Inline door entities can carry class links, ordered entity links and a manual loadout. | [#1129](https://github.com/p4kx-org/p4kx/issues/1129) retains these scene facts and preserves container-local identity, absent versus empty overrides, controller and sensor references. [Room registry](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/socpak/room_volume.rs) · [Entity links](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/socpak/entity_links.rs) |
| P4KX [currently selects one light mode](../crates/p4kx-model/src/soc_light.rs). A nonzero auxiliary mode can supersede an authored default of zero. StarBreaker retains modes, inherits sparse class defaults and resolves tile-group placement. | [#1130](https://github.com/p4kx-org/p4kx/issues/1130) retains the raw states and class/port evidence. Prove the preview behavior before changing it. [Class defaults](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/socpak/light_component.rs) · [Port anchoring](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/socpak/light_anchor.rs) |
| The player kit joins body defaults, authored loadouts, ports, controller scopes, weapon and movement tag variants. | [#1131](https://github.com/p4kx-org/p4kx/issues/1131) adds source-backed joins and may reuse the character package. Authored variant sets do not establish runtime selection. [Player export](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/character/player/export.rs) · [Motion graph](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/character/player/motion.rs) |
| DDS declarations distinguish transfer encoding from filename or material role. StarBreaker also offers an opt-in, topology-limited DDS package writer. | [#1132](https://github.com/p4kx-org/p4kx/issues/1132) handles source and converted transfer semantics. [#1135](https://github.com/p4kx-org/p4kx/issues/1135) handles optional block reuse and explicit fallback. [Header](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-dds/src/dds_file.rs) · [PNG conversion](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/pipeline/textures.rs) · [DDS package writer](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-dds/src/package.rs) |
| Material source and entity effect declarations include emission, alpha coverage, forward cards and exact Glow target/modifier links. | Extend [#1097](https://github.com/p4kx-org/p4kx/issues/1097) and [#1109](https://github.com/p4kx-org/p4kx/issues/1109) for raw control vocabulary. [#1133](https://github.com/p4kx-org/p4kx/issues/1133) owns typed joins and evidence-gated projection. P4KX already normalizes palette glossiness at consumption. [Material semantics](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/material_semantics.rs) · [Glow joins](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/pipeline/engine_glow.rs) |

StarBreaker's [stamp](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/export_stamp.rs) is attached
to generated UI artifacts when present. The adjacent build ID or sorted
directory digest identifies that input context. It does not hash every original
payload or every package member. P4KX package validation can reuse existing
artifact receipts when available. The upstream cancellation checkpoints also
provide a useful mid-write cancellation fixture for the existing shared writer
and package cancellation path.

The bounded installed DDS witness read only prefixes of up to 148 bytes. It
checked 2,048 general headers and 256 diffuse-suffix headers in each of LIVE and
PTU. In both builds the diffuse stratum contained Cry marker entries with
the sRGB bit both clear and set. There were also marker/flag-positive entries
without an explicit sRGB DXGI format. The declaration and usage must remain
independent. The [DXGI definitions](https://learn.microsoft.com/en-us/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format),
[Cry header definitions](https://github.com/aws/lumberyard/blob/master/dev/Code/CryEngine/CryCommon/ImageExtensionHelper.h)
and [glTF material rules](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#materials)
provide separate checks for original declaration and consumer encoding.

The GLB check exercised the preflight function in Blender 5.3's bundled Python
with a synthetic stream. It did not import the invalid model. The remaining
StarBreaker findings identify implementation seams and source tests for their
owning tickets. They are not claims of installed-PTU parity.

### Follow-up against the integration branch

The same StarBreaker head was checked against P4KX integration commit
`1b0ea6a29`. [GLB/package preflight](https://github.com/p4kx-org/p4kx/issues/1134)
and [DDS transfer encoding](https://github.com/p4kx-org/p4kx/issues/1132) are now
implemented in the integration branch. The remaining source and package work
uses the existing tickets above, with two additional implementation units.

- [#1136](https://github.com/p4kx-org/p4kx/issues/1136) tracks the physical default
  material, its provenance and its selection by fallback slots. The virtual-root
  repair in #1091 resolves a model's authored request. The physical default is a
  separate input for slots that still need a fallback. Keep the source mesh ID,
  source library position and dense GLB/package material index distinct.
- [#1137](https://github.com/p4kx-org/p4kx/issues/1137) tracks DDS alpha association
  and selected-mip transparency. P4KX normalizes DXT2/3 to BC2 and DXT4/5 to BC3.
  The shared block layouts still carry different alpha association declarations.
  The model texture loader's alpha-format list omits BC1 and BC2. An in-bounds
  BC1 transparent selector or a BC2 alpha sample can therefore survive decode
  while the loader reports no alpha channel. Keep the source declaration, observed
  coverage and consumer interpretation separate, including data packed in alpha.
  [Microsoft legacy format mapping](https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-legacy-formats)
  · [BC1 selectors](https://learn.microsoft.com/en-us/windows/win32/direct3d9/opaque-and-1-bit-alpha-textures)
  · [DX10 alpha modes](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header-dxt10).
- For SOC doors, StarBreaker assigns a class-derived controller to a shared
  geometry entry. A shared mesh can represent placements of different classes.
  Keep class identity, controller references and inline loadouts per placement
  in [#1129](https://github.com/p4kx-org/p4kx/issues/1129). Sensor volumes and
  room containment represent different relationships.
  [StarBreaker interior writer](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/pipeline/interiors.rs).

### Package validation and identity

Rechecked against P4KX integration commit `d6d665694`. A synthetic package
declared `authored/soc-scene.json` and passed the standalone structural check
both when the file was missing and when it contained an invalid document. The
Blender importer already loads the companion before creating scene objects.
[Issue #1138](https://github.com/p4kx-org/p4kx/issues/1138) covers standalone
validation and digest coverage for the companion.

The package manifest records exporter version, options and the archive Build
when available. The [standalone validator](blender-package.md#validate-a-package-without-blender)
can check a supplied SHA-256 inventory. Native fixture workflows already
produce such an inventory and record the exact exporter binary digest.
Regular package exports lack an equivalent inventory. #1138 uses the existing
receipt format to bind the files in the declared package profile, including
the SOC companion. Exporter identity, archive identity and output-byte
consistency are separate facts. An archive directory fingerprint identifies
directory evidence only. A receipt from the writer is a consistency check.
[StarBreaker export stamp](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/export_stamp.rs)

### Attachment classification on scene instances

StarBreaker exports the authored item type on each decomposed scene instance.
P4KX already reads the same `AttachDef.Type` and `AttachDef.SubType` fields for
Entity resolution and exposes them through catalog v3. The Assembly parts
and Blender scene nodes discard the values. A package consumer needs a
separate catalog lookup to classify installed parts. [Issue #1139](https://github.com/p4kx-org/p4kx/issues/1139)
carries the exact selected fields and their source evidence through Assembly
to the corresponding package entity node.
[StarBreaker scene contract](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/docs/decomposed-export-contract.md)

### Split DDNA and current implementation

The recheck uses the same open StarBreaker head and P4KX integration commit
`b6beb5416`. P4KX now has GLB preflight, DDS transfer and alpha handling,
opt-in DDS packaging, deterministic file receipts, room and door data,
physical default-material selection and authored item classification. The
integration branch also anchors direct vehicle tile light groups to the
unique owner helper. Package light origins and consumer verification remain
in [#1130](https://github.com/p4kx-org/p4kx/issues/1130).

The package DDS loader refuses a split tail before testing the selected DDS
members separately. The PNG path preserves the shared selected mip and merges
the tail into the normal image. StarBreaker can publish two DDS assets with
copied payloads. [#1140](https://github.com/p4kx-org/p4kx/issues/1140) owns that
route in P4KX, with independent per-artifact provenance and a documented PNG
fallback. A synthetic DX10 BC4_UNORM image in headless Blender 5.3.0 returned
the expected size and red sample. The tested DX10 R8_UNORM and A8_UNORM
images returned zero size and no pixels. The probe checked image loading.
Material-graph and installed-source equivalence remain acceptance checks.
[Current P4KX DDS loader](../crates/p4kx-extraction/src/model_texture_dds.rs)
· [StarBreaker texture pipeline](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/pipeline/textures.rs)

For animation, the positioned Media rows capture definitions and declared
database links. [#1094](https://github.com/p4kx-org/p4kx/issues/1094) owns
ordered `#filepath` scope, exact CAF bindings and event timelines.
[#1095](https://github.com/p4kx-org/p4kx/issues/1095) owns the typed Mannequin
and blend-space graph. Player variants, weapon motion and package projection
stay in [#1131](https://github.com/p4kx-org/p4kx/issues/1131). P4KX retains the
original declaration and physical DBA index independently of optional pose
selection. Upstream's default-pose export evaluates a caller-selected clip.
[StarBreaker directives](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/chrparams.rs)
· [StarBreaker default pose](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/pipeline/default_pose.rs)

For collision, retain each original shape, its placement and the actual
affine transform independently. A transformed sphere or capsule can have
non-spherical or non-uniform cross sections under a non-rigid transform.
Keep missing and ambiguous references visible when following CDF and vehicle
grid targets. This is part of [#1128](https://github.com/p4kx-org/p4kx/issues/1128).
[StarBreaker collision export](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/decomposed_physics.rs)

P4KX already tests cancellation while a texture is loaded and preserves a
previous published package. Keep the same staging check when adding a linked
asset pair. [P4KX package writer tests](../crates/p4kx-blender/tests/it/package_write_context.rs)

### Global image indexes and multi-rig clip identity

The upstream PR was still open at `14ae2d99` on the follow-up date.
P4KX integration commit `b9d9cc6f3` resolves concrete CAF paths against all
authored tracks-database declarations. Its candidates identify the physical
Archive Entry, DBA clip ordinal and original byte intervals. The separate
loose-file lookup remains available, with incomplete bank coverage explicit.
The current integration also carries typed Mannequin and blend-space source
nodes and physical collision skeleton metadata.
[Upstream contract](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/docs/decomposed-export-contract.md#L740-L803)
· [P4KX binding change](https://github.com/p4kx-org/p4kx/commit/b9d9cc6f3)

CRC and chunk-directory checks passed for both global image files in the
installed LIVE and 5.0 PTU. `Animations.img` contains a `0x370275B7` chunk at
version `0x902`. An independent walk reconciled its eight-byte prefix,
308-byte records, first 256-byte path fields and eight trailing bytes in both
builds. `DirectionalBlends.img` contains a `0x6B8DF259` chunk at version
`0x901`. The counted path audit found AIM entries separated by variable
distances. Their full record boundaries and pose semantics remain unverified.
The current animation provider admits CAF, DBA and AIM only. [#1141](https://github.com/p4kx-org/p4kx/issues/1141)
tracks both image indexes, their raw spans and source-proven references.
[CryEngine CAF header description](https://www.cryengine.com/docs/static/engines/cryengine-5/categories/23756813/pages/23306441)
· [CryEngine directional blending description](https://www.cryengine.com/docs/static/engines/cryengine-5/categories/23756816/pages/25535363)

P4KX `AnimationListing` reads model-adjacent clip headers without decoding
their keys. The combined entity view in [#1093](https://github.com/p4kx-org/p4kx/issues/1093)
still needs to retain root and child rig contributors together. Grouped
presentation can keep each physical clip, frame rate and controller reference
independent. A synthetic check should show two unrelated same-name clips
with different rates and fragment references remaining separately addressable.
StarBreaker's merged view groups by name and retains the first contributor's
fragment list. Its optional final-frame pose illustrates why a channel with
no samples must retain its bind component and why axis conversion must occur
once. Any future opt-in bake needs an exact source clip, a requested sample
and separate derived output. Raw clip flags, Mannequin playback settings and
derived comparison of clip endpoints remain independent facts.
[Upstream merge limitations](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/docs/decomposed-export-contract.md#L767-L803)
· [Upstream pose sampler](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/pipeline/default_pose.rs)
· [P4KX header listing](../crates/p4kx-extraction/src/animation_listing.rs)

### Integration recheck on 2026-11-14

GitHub still reported the open upstream PR at `14ae2d99`, with 32 commits
and 123 changed files. The P4KX comparison used the pushed integration
checkout at `9ebb9d655`.

- Static Entity and Interior packages can include an optional vertex map
  linking each GLB POSITION accessor to the physical model and stream,
  requested and actual LOD, and available original source indices and
  origins. A second export to the same destinations produced identical map
  digests. Blender 5.3 imported the replacement package with verified digests
  and no warnings. Rigged mapping and full model metadata acceptance remain in
  [#1092](https://github.com/p4kx-org/p4kx/issues/1092).
- The comparable upstream GLB integration test checks emitted material-slot
  order against the associated material description. P4KX accepted a
  structurally valid GLB with two transposed `p4kx_index` tags and applied
  the two material descriptions to the wrong slots. Static package preflight
  now checks explicit tags against their actual array indices. Older
  untagged meshes retain the existing name fallback. Blender 5.3 verified
  matching same-name indices and skipped the affected mesh when the tags
  were transposed. [StarBreaker GLB integration test](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/tests/decomposed_glb.rs)
  · [P4KX validation change](https://github.com/p4kx-org/p4kx/commit/9ebb9d655).
- The palette comparison exposed another default-value problem. The Blender
  graph treated an explicit zero layer `GlossMult` as the neutral multiplier
  of one. The synthetic palette fixture now yields a roughness input of one
  for that layer, in agreement with the P4KX GLB factor path.
  [Upstream palette source](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/pipeline/palette.rs)
  · [P4KX gloss change](https://github.com/p4kx-org/p4kx/commit/6856c157a).
- The animation provider publishes both global image sources with their raw
  payloads and verified path spans. A CAF-to-DBA clip edge requires the original
  bank selector and a matching full path in the physical bank metadata.
  An encrypted bank can therefore mark a joined clip field as encrypted even
  when the index path and hash themselves came from an unencrypted source.
  Full directional record framing remains unverified.
  [#1141](https://github.com/p4kx-org/p4kx/issues/1141)
  · [P4KX field provenance change](https://github.com/p4kx-org/p4kx/commit/6ca661ba2).

The upstream room-containment helper tests geometric inclusion for a point
in a container-local segment. It does not add authored room membership to
the export. P4KX exports the segment geometry and source GUID links in
[#1129](https://github.com/p4kx-org/p4kx/issues/1129). A consumer can compute
inclusion from those inputs when it needs it.
[Upstream containment helper](https://github.com/diogotr7/StarBreaker/blob/14ae2d9979af1f39f0299ccda1933e0f988beaa2/crates/starbreaker-3d/src/socpak/room_containment.rs).

### Merged source verification

GitHub merged PR #54 as [`305215bca`](https://github.com/diogotr7/StarBreaker/commit/305215bca75350013c221bdfbcb2f8180fe09dbe).
Both the merge and the audited head `14ae2d99` resolve to Git tree
`294754f1b9e0951ee00e3e3a57227fa7995e10fd`. The squash changed the commit
identity while keeping the source tree identical. The upstream runtime
evidence in the PR covers its named LIVE build. GitHub still lists
[v0.3.2](https://github.com/diogotr7/StarBreaker/releases/tag/v0.3.2) as the latest
published release. Use the merge commit when identifying a matching source build
for a differential comparison.
[Pull request](https://github.com/diogotr7/StarBreaker/pull/54)

The integration recheck at `2796853dd` found a concrete join still missing
from [#1092](https://github.com/p4kx-org/p4kx/issues/1092). A static package
lists the original geometry material IDs together for each dense GLB material.
For a single-material library or the archive fallback, multiple source IDs can
share a single dense index. Each emitted GLB primitive carries only that dense
index. The layout drops the source submesh ordinal while selecting valid,
visible primitives. The vertex map binds the mesh and POSITION order to the
physical source. It contains no primitive binding. Rigged meshes keep their
materials inside the GLB and need the same primitive-level link.
[P4KX layout](../crates/p4kx-gltf/src/layout.rs) · [GLB writer](../crates/p4kx-gltf/src/document.rs) · [Package mesh types](../crates/p4kx-blender/src/manifest_mesh.rs)

Preserve the source row identity from layout selection through emission.
Use the emitted GLB mesh and primitive ordinal and optional dense material index
as the output coordinates. Resolve the physical model entry, chunk and source
submesh row independently from the material library and slot used for drawing.
The existing Model Media export already retains the source row ID, node ID,
material ID and first-index/count fields with their original evidence. Reuse
those coordinates and the field-origin contract. Keep absent or unavailable
identities explicit when a transformation loses the correspondence. Verify
sparse and repeated slots, a hidden preceding submesh, shared fallback,
deduplicated instances and rigged parts. Validate geometry/material assignment
in Blender separately from the source evidence.
[Model Media submesh rows](../crates/p4kx-media/src/model_source/mesh_submeshes.rs) · [StarBreaker material order](https://github.com/diogotr7/StarBreaker/blob/305215bca75350013c221bdfbcb2f8180fe09dbe/crates/starbreaker-3d/src/gltf/glb_builder.rs)

The remaining comparison work already has owners. The lazy clip inventory and
independent rig contributions stay with #1093. Scoped animation graph, event
bindings and player variants stay with #1094, #1095 and #1131. Physical
collision fidelity and typed emission remain in #1128 and #1133. The merge
introduced no additional archive, audio or DataForge format change beyond the
previously reviewed tree. The release and source revision remain separate
producer identities for [#1104](https://github.com/p4kx-org/p4kx/issues/1104).
