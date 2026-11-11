# Installed PTU media compatibility and export audit

Research date: 2026-11-06. Source baseline:
[1ee9c0edbfc34e616d2f140951e4eb5bc1c67558](https://github.com/p4kx-org/p4kx/tree/1ee9c0edbfc34e616d2f140951e4eb5bc1c67558).

This extends the [DataForge/catalog compatibility audit](ptu-export-compatibility-audit.md)
and [scunpacked export comparison](scunpacked-data-audit.md) to models, materials, textures,
animations and audio in the installed 5.0 PTU. The installed LIVE Archive provides a comparison.

Only technical inventory/coverage, source structures and implementation findings appear here.
No extracted game assets, concrete private PTU asset/Record identities, keys, source dumps or
gameplay values are included. Publishing technical issues was explicitly authorized.

## Result

No fundamental media container/codec format break was reproduced in the exercised PTU readers.
One existing product defect was confirmed in both installations: material references with
virtual-root or authoring prefixes can reject a library that exists in the Archive.
Two independent test defects were also established: fixed Entity fixtures and texture
output checks that omit supported array/volume naming.

The audit produced **15 GitHub issues**: one overview and 14 implementation tickets.
All belong to milestone **v0.5.0**, carry `ready-for-agent`, and have concrete source seams,
data contracts, compatibility requirements and verification/acceptance criteria.
There are three bug tickets and eleven implementation enhancement tickets;
the overview itself is labeled enhancement.

Overview: [#1088](https://github.com/p4kx-org/p4kx/issues/1088).

## Method

Release test binaries and CLI were freshly built from the pinned checkout with
`--locked --offline`; stale pre-existing executables were not used as conversion evidence.
Small independent probes used the same reader crates without changing repository implementation.

The game installations were read only. All builds, private probes, logs and extracted artifacts
stayed in the task directory outside the repository.
Stored-key helpers supplied the Archive key without printing it. Launcher credentials were not read.

The structural census reads 17 recognized chunk-bearing extensions, recurses through nested
socpaks and sniffs other extensions. Semantic/model/texture/audio checks then exercise actual
readers and output writers. A parser pass is distinguished from complete source-field export.

## Coverage and evidence

| Area | PTU checks and result |
| --- | --- |
| Chunk containers | 987,797 files, including 10,584 top-level socpaks and nested content; zero read failures or size skips. LIVE: 955,057 files and 9,668 socpaks. Both expose the same 58 container/type/version combinations. |
| Model geometry | Complete mesh-stream census: 396,849 geometry files / 396,734 mesh chunks, no stream overflow/read failures. Main stratified conversion check: 901 valid GLBs across CGF/CGA/skin/CHR/CrCh VVG. Additional LOD, UV, tangent, winding and CrCh bounds checks passed. |
| Skin/skeleton | All 20,339 skins read with their LODs. Skeleton-as-node, skin/joint and sampled per-LOD GLB checks passed. |
| Animation binaries | All 54,391 CAF and 546 DBA files read: 70,312 decoded clips. Header/decoded metadata agrees; observed key encodings are supported. |
| Animation declarations | All 8,874 chrparams/animevents/ADB/bspace/comb files extracted, CRC-checked and parsed as XML. AIM files are structurally inventoried; no AIM semantic decoder is claimed. |
| Material libraries | All 26,307 libraries / 203,020 materials parsed; no parser truncation. Raw PublicParams, MatLayers, texture modifiers, attributes and unknown elements are populated. |
| DDS inventory | All 48,719 main and 7,773 alpha headers checked. No main part-count anomalies, malformed headers or orphan/headerless alpha layouts. 24 main formats observed; R32_UINT occurs in PTU and its current decoder works. |
| Texture conversion | 1,045 unique sources fully merged across their stored mips; selected-mip decoding produced 31,830 images without error. Covers all main/alpha arrays, volumes, cubemaps and HDR/float sources. All 381 HDR/float sources have correct image counts and finite RGB/available alpha on the selected mip. |
| Full-resolution texture repros | Two actual extraction cases produced all 128 volume slices and 64 array layers as valid PNGs with expected dimensions. Repeating IfChanged skips correctly. Existing all-float OpenEXR and cubemap/alpha/order checks also passed. |
| Audio structure | All 2,410 banks, all bank version 154, parse. All supported HIRC types decode; no malformed supported object layouts. Survey covers 120,865 loose/embedded WEM headers; all observed format tags are Wwise Vorbis. |
| Audio conversion | 1,515 sampled WEM OGG/WAV conversions passed; 2,011 WAV-versus-decoded-OGG comparisons agree; 6,242 sampled rebuilt Vorbis streams decode fully. Nineteen targeted representatives cover every observed channel/rate bucket, including 12 channels, without error. |
| Audio integration | Index builds with no issues; localization, streamed/localized WAV export, extraction and TXTP checks pass. Full ATL census reconciles trigger counts and proves non-trigger controls are populated. |
| Integrated geometry | Independent five-class probe: small/medium/capital ship, ground vehicle and weapon; Colours and Textures modes produce valid GLBs. Worn armour, body/facial animation and every declared creature-database GLB check pass. |
| CLI package output | Actual ship, armoured character and creature exports pass package schema and repository GLB checks. |

Texture pixel checks select a mip whose largest dimension is at most 256 for the comprehensive
indexed probe. All stored mip bytes are merged, but this is **not** a full-resolution pixel decode
of every mip of every texture. The full-resolution repros and float EXR checks supplement it.

One unindexed texture batch was stopped during an expensive full-archive repeated lookup.
Seven checks had passed and two had reproduced false test failures; unfinished tests are not
counted as passed. The indexed exhaustive complex/HDR probe replaces that coverage explicitly.

## Confirmed defects and distinctions

### Material reference resolution

`p4kx-model/src/mtl_load.rs::request_path` rejects both a virtual-root source reference and an
authoring-prefixed reference before an Archive resolver can find the existing Data-relative library.
A minimized probe loaded both safe Archive targets, applied their libraries, wrote valid GLBs and
verified changed output from the fallback versions.

The same affected models also fail on LIVE. The LIVE sampled test passes because it chooses
different entries. This is an **existing resolver gap exposed by the PTU audit**, not a new PTU-only
regression. The issue retains strict filesystem/network/traversal boundaries and original references.

### Canary fixture drift

The assembly GLB canary and two audio canaries stop on an unavailable preferred Entity.
An independent capability/family-based selection completed the remaining cases. This proves the
test dependency is stale; it does not justify changing production exact Entity lookup into fuzzy matching.

### Texture output false negatives

`texture_conversion.rs::has_image` recognizes unsuffixed or posx output only. Arrays/volumes use
layer/slice output names. Actual extraction, complete PNG decoding and repeat/conflict checks pass.
The required fix strengthens complete artifact-set validation instead of weakening conversion assertions.

## Additional export data

The new common contract is a bounded, deterministic `p4kx media export` pipeline with domain
providers, a versioned index/JSONL layout, raw sidecars, source/subresource provenance, coverage/status
and atomic output. It complements raw extraction, gameplay catalog export and Blender packages.

- Models: original node/LOD/skeleton/source mappings, stream/quantization provenance, complete chunk
  directories and unconsumed physics/morphology/visibility payloads without invented semantics.
- Animations: source frames, controller IDs, full flags, additive/unbound tracks and complete
  CAF/DBA/AIM inventories; character bindings, event timelines, Mannequin fragments/transitions and
  blend-space declarations. Source export does not require solving additive composition.
- Materials/textures: complete retained material inputs and unknown fields; raw DDS headers,
  mip/alpha/array/face/slice topology and mappings to derived images.
- Audio: complete retained BNK/WEM facts and undecoded bus/attenuation/FX/modulator payloads;
  shared node properties, RTPC/state curves, routing/positioning; selection/blend/transition conditions;
  complete ATL RTPC, switch/state, environment, preload/external-source and event metadata.

Existing `bnk objects`/MCP music JSON (#860/#870), all-RIFF chunk access (#279), raw chunk dump
and full Blender material descriptions (#158) already solve parts of this problem. The tickets reuse
these rather than falsely reporting them as absent.

## Issues and implementation order

| Issue | Work |
| --- | --- |
| [#1089](https://github.com/p4kx-org/p4kx/issues/1089) | Shared streaming export contract and provider pipeline |
| [#1090](https://github.com/p4kx-org/p4kx/issues/1090) | Capability-based real-Archive canary fixtures |
| [#1091](https://github.com/p4kx-org/p4kx/issues/1091) | Archive material virtual-root/authoring-reference resolution |
| [#1092](https://github.com/p4kx-org/p4kx/issues/1092) | Model/LOD/skeleton/source chunk metadata and opaque payloads |
| [#1093](https://github.com/p4kx-org/p4kx/issues/1093) | Animation tracks, raw flags, complete CAF/DBA/AIM inventory |
| [#1094](https://github.com/p4kx-org/p4kx/issues/1094) | Character bindings and animation-event timelines |
| [#1095](https://github.com/p4kx-org/p4kx/issues/1095) | Mannequin fragments/transitions and blend-space graphs |
| [#1096](https://github.com/p4kx-org/p4kx/issues/1096) | Complete array/volume/cubemap conversion output checks |
| [#1097](https://github.com/p4kx-org/p4kx/issues/1097) | Complete raw material libraries, layers and texture modifiers |
| [#1098](https://github.com/p4kx-org/p4kx/issues/1098) | DDS source headers/topology and artifact mappings |
| [#1099](https://github.com/p4kx-org/p4kx/issues/1099) | BNK/WEM metadata, complete retained fields and opaque data |
| [#1100](https://github.com/p4kx-org/p4kx/issues/1100) | Shared node properties, RTPC/state curves, routing and positioning |
| [#1101](https://github.com/p4kx-org/p4kx/issues/1101) | Container selection, blend curves and music decisions/transitions |
| [#1102](https://github.com/p4kx-org/p4kx/issues/1102) | ATL controls, preloads, external sources and event metadata |

All implementation tickets are native sub-issues of #1088.
Metadata providers depend on #1089; behavior graphs reuse #1094 source-tree support;
audio extensions depend on #1099 and shared node parsing #1100 where applicable.
Bug fixes are independently actionable.

Existing #1006/#1087 attachment/alignment, #668 additive composition, #627 facial convention,
#979/#775 livery and #1035 filesystem case handling remain their own work. No new claim of
in-game appearance or acoustic correctness is made.

## Reproduction

Use PowerShell and the approved agent scratch folder for TMP/TEMP and CARGO_TARGET_DIR.
Set P4KX_DATA_P4K to the installed PTU Data.p4k, then repeat comparison checks with LIVE.
The ignored test helpers obtain the stored key.

```text
cargo test --release --locked --offline -p p4kx-extraction --test real_archive --no-run
cargo test --release --locked --offline -p p4kx-assembly --test real_archive --no-run
cargo test --release --locked --offline -p p4kx --test real_archive --no-run
```

Run the built real_archive binaries with `--ignored --nocapture` and the following filters:

- Extraction: `chunk_types::`, `model_verification::`, `animations::`, `skeletons::`,
  `materials::`, `mesh_streams::`, `model_lods::`, `texture_conversion::`,
  `audio_survey::`, `soundbank_hirc::`, `audio_verification::`.
- Assembly: `real_assembly_verification::`, `real_head_animation::`,
  `real_creature_animations::`, `real_wwise_verification::`, `real_streamed_localised::`.
- CLI: `a_coarser_texture_mip_makes_a_ship_package_much_smaller`,
  `a_character_with_armour_is_one_skinned_package`,
  `a_creature_with_its_skeleton_exports_to_a_valid_glb_and_package`.

The material, fixture and texture-output failures above are expected before their fixes.
The private independent probes are not shipped as repository tests or game fixtures; each issue
specifies synthetic regression fixtures and an ignored installed-Archive check for implementation.

## Limits

There was no in-game reference render, animation convention comparison or acoustic playback comparison.
Most model conversions are stratified samples, supplemented by full stream/skin/animation inventories.
Opaque physics/cloth/aim/other chunks remain opaque; inventory is not semantic decoding.
No real PTU Opus/ADPCM assets were present, so this audit does not establish their real-Archive behavior.
Existing preview budgets and TXTP approximations remain in place and must be explicit in future exports.

No implementation code was fixed or committed during this audit. The source changes requested
by the user are represented by the linked issues; temporary binaries, extracted assets and probes
are removed after verification.
