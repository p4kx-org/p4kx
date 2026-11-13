# Complete material source metadata

`p4kx media export ARCHIVE --out DIR --kind material` inventories selected `.mtl` Entries directly. A model or geometry conversion is unnecessary, so unused libraries are included. Normal publication requires complete advertised projection; `--diagnostic` retains explicitly partial/failed records using the common media publication contract.

## Bytes and structured source

The common record `source.raw` identifies the exact original decoded MTL Entry bytes with its all-byte digest and length. Plain XML is not decoded with replacement characters. Invalid UTF-8 produces a typed failed structured projection while original bytes remain independently retained. CryXml strings retain the reader's established repair behavior and expose an actual `repaired_utf8` fact; repaired structured data is partial. Normal CryXml decoding, XML escaping or an owned allocation alone is not evidence of repair.

The authored tree preserves the material-set root separately from child materials, including ordered attributes, PublicParams, SubMaterials positions, unknown entry dispositions, MatLayers, texture attributes, TexMod blocks and unknown nested elements. Root flags/parameters are not inherited into every child. All original material/child ordinals remain original when a bound omits another node.

`syntax` contains original QName, ordered attributes and namespace declarations. Plain XML namespace URIs come from the validating XML reader. For CryXml the QName/declaration strings are retained; absent expanded URIs make no namespace-resolution claim. Plain text `lexeme` strings come from original UTF-8 input and include entity/CDATA spelling; attribute lexemes exclude surrounding quotes. Binary attributes have no text-XML lexeme, and `binary_text` is independent of children because the binary node does not encode mixed text/child interleaving. No field in this projection claims an offset into original binary MTL bytes. XML prolog and byte-level syntax remain in `source.raw`.

## Existing interpreted fields

Each material keeps its original index, raw parameters/layers/texture slots/unknown blocks and existing interpreted surface values. `interpreted_surface.basis` is `existing-reader-with-defaults`: its Float32 bits identify the interpreted f32 values, including existing defaults. Original shader, generation-mask, flag and numeric tokens remain strings in the authored source tree. A malformed or nonfinite XML token is not claimed to survive as an exact source JSON float. Palette/tint decisions are separate from original attributes.

Layer Path and Submtl, texture filenames/runtime placeholders and the default-palette token remain original declarations. Layer/texture file targets are looked up only in the immediately owning Archive with the existing safe MTL/DDS path rules. Complete inventory can prove Missing, actual duplicate physical candidates remain Ambiguous, and filtered targets remain NotSelected. Recovery cannot turn an unseen target into Missing. Original Submtl remains a name, separate from the library identity.

Resolved proves physical identity, not readability or successful projection. Actual target Failed/Partial records or the original fatal read error retain that separate evidence. Every selected physical library is read once in the normal walk; cyclic libraries retain both actual declaration edges without recursive companion opens or another target parser. DefaultPalette remains Unavailable(TargetNotRead) without DataForge Record evidence; runtime texture placeholders preserve their role and unresolved binding. No palette GUID, shader execution or in-game appearance is inferred.

## Bounds and cancellation

`MaterialProvider::new_with_limits` accepts domain limits in addition to the common generation limits. The Rust 0.5 configuration field `MaterialSourceLimits::elements` is `Option<usize>`: default `None` imposes no authored element-count bound; `Some(n)` requests a finite bound that includes the root and retains explicit `Elements` loss when reached. `Some(0)` is `InvalidSourceLimits`. This policy is shared by the existing parser, provider, checkpoint and observer routes. Defaults retain 16 MiB structured-input admission, 4 MiB canonical syntax/text storage and depth 32. Maximum configurable recursive source depth is 64. Input is bounded before DOM parsing; source reads poll cancellation/change checks per chunk, and structured capture/material passes preserve original checkpoint errors.

Canonical-tree limits are distinct from existing per-material typed reader bounds. The rendering parser's existing cardinality/RawElement/layer/texture semantics remain intact; the legacy `MaterialLibrary::parse` API acquires no new input-byte cap. Source depth/element/text cuts and existing raw-field truncation are explicit metadata/diagnostics. These are retained-structure budgets, not a claim that XML parser/indexing transient memory is limited to four MiB. Actual record/value-depth/output limits remain enforced by the common writer.

Cached path-index work shares the generation-wide metadata_work bound with metadata cursor/read work. Key hashing, stable linear sorting, binary search and every visited hash bucket poll the actual cancellation/change checkpoint. The index is built once per needed owning source without target payload reads; source-byte and physical Entry counters keep their meanings. Simultaneously active key storage and source admission stay bounded, including nested work completed before a first parent lookup.

## Consumers and versions

The material provider is version 1.1.0 within the existing media 1.0 envelope. Its owned library/failure namespaces have typed schema constraints; custom provider namespaces retain their generic contract. `model info --json` keeps all existing fields and adds shared authored metadata under library/material descriptions.

Blender package format 1.9 adds `material_libraries`, once per distinct retained reader path/authored root, and each material's `source_library` link. Existing material index/single semantics, graph inputs, palettes, GLB values and older raw fields keep their meaning. Equal source-object pointers are only a private deduplication shortcut backed by retained ownership; separate objects require complete path/content equality. Different authored roots with one renderer identity both survive and are diagnosed as ambiguous. These are source descriptions, not physical Archive identity proofs; media records carry physical provenance. Older manifests omit the new fields and remain readable. The repository importer recognises minor 1.9 and leaves rendering behavior on the established fields.

## Verification boundary

Independent synthetic cases cover roots, unknown submaterial slots, ordered case-distinct/namespaced attributes, mixed text/CDATA/comments/PI including endtag-like data, layers/TexMod/placeholders, strict UTF8 and true CryXml repair, truncation, original ordinals, raw bytes/digests, cancellation/publication and schemas. The Extraction census compares retained root-Entry fields with the shared wire shape in memory.
The opt-in Media replay additionally runs the actual material provider and reconciles its JSONL
records, verified raw artifacts, original container/Entry identities and coverage counts against
a separate read of all selected root and nested `.mtl` Entries. It reports only aggregate counts
and keeps its temporary generation outside the input installation. Set `P4KX_DATA_P4K` and optionally
`P4KX_DATA_P4K_B`, then run
`cargo test -p p4kx-media --test real_archive all_selected_material_libraries_and_raw_artifacts_reconcile -- --ignored --nocapture`.
Diagnosed structured failures or losses remain separately counted. A traversal or accounting
failure cannot pass as a complete replay.

Blender import/render success alone does not prove source completeness or game semantics. Source-built Blender 5.2 verification must inspect unchanged supported node groups/links/color spaces/inputs and warnings, after exact metadata and raw-digest checks.

## Shader-name and selected-flag references

`shader_references` is an additive material-library field. Its shader and flag roles are separate
from existing layer/texture/palette references. Original authored attribute positions and selected
token positions survive, including duplicates, empty Shader attributes and unresolved names. No
reference is fabricated from an absent Shader attribute or an engine-default material slot.
Each reference retains a common explicit resolution plus an optional actual `InputReceipt`.

Selecting both material and shader enables unique source/Property matches across nested containers.
Filter and kind exclusion remain explicit NotSelected states; excluded source bytes are never
opened. Incomplete lookup scope remains Unavailable, not Missing. The same-name candidates from
multiple physical sources are retained rather than choosing first or preferring exact case.
A cached parsed source is bound to its observed root input, and receipts identify the specific MTL
attribute/token use. `reference-lookup` denotes a private verified read, not another raw publication.
See [shader source projection](media-shader-backend.md#material-declaration-links) for the bounded
matching rules, unknown global-flag scope and the exact verification boundary.

These links do not change MTL loading, GLB conversion, Blender shader groups, numeric GenMask
interpretation or rendered appearance. Missing/ambiguous/unavailable links preserve source facts
rather than selecting a shader at runtime.
