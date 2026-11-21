# Shader source projection

`p4kx media export ARCHIVE --out DIR --kind shader` exports source extension declarations and
opaque compiled-cache inventory. The default registry includes the permanent Archive provider and
`ShaderProvider`; the library also accepts an explicit `ProviderRegistry` through `export_with`.
The material provider adds source-bound shader/flag references when material output is selected.

The stateless provider reads only the retained payload supplied in `SourceContext`; it has no
Archive handle, companion lookup or host-path fallback. Shared traversal supplies ordered nested
container identity, raw payload artifacts, integrity facts, selection, cancellation and publication.
The format reader's separate source/token/value/count/depth limits bound one active source.

## Source representation

One `.ext` source produces a definition header plus one row per ordered declaration. Top-level
braced Properties have a separate payload tag/namespace and an original Property ordinal.
Every declaration retains its complete original ordinal path, immediate parent and child count.
Unknown fields, unknown nested scopes and repeated names remain separate rows.

Byte ranges use the original decoded Entry as their origin and have end-exclusive bounds.
UTF-8 BOM, comments, quoted tokens, escapes and whitespace remain available in the exact retained
source. Name, value and sole-literal ranges carry bounded copies of their original text; a block's
entire body is not copied into each row. Physical ranges and raw hashes are verified independently
of JSON Schema.

Version, common flags, Property names/labels/descriptions, dependencies and Hidden occurrences
retain their authored forms. Absent fields create no invented occurrence. Bare markers have no
value; explicit empty values stay present. A filename stem is labelled as the display-name origin.
An apostrophe between ASCII letters belongs to the unquoted word. Single- and double-quoted
tokens still keep their closing delimiter and escapes.
No version grammar, common-flag inheritance, escape interpretation or dependency path is inferred.

## Interpretation and counting

Supported `0x`/`0X` unsigned 64-bit literals and the unambiguous zero token are exported as exact
canonical decimal strings. JSON floating-point precision cannot discard mask bits.

| Source observation | Record status | Diagnosis |
| --- | --- | --- |
| Supported explicit literal | Complete | Exact interpreted word |
| Unknown radix, unsupported expression, missing/empty value | Complete | Info; raw source remains complete |
| Value beyond the supported unsigned64 interpretation width | Complete | Info; no engine-invalidity claim |
| Malformed explicit hexadecimal literal | Partial | Warning within the declared literal profile |
| Unknown future reader outcome | Partial | Warning; no guessed interpretation |
| Encoding, structure or domain-limit failure | Failed | Error; raw retained, structure counts unavailable |
| `.cfib`, `.cfxb`, `.shcb` cache inventory | Complete | Info; semantic decoder explicitly unsupported |

Definition, Property, declaration and compiled-cache counts remain distinct. A compiled cache
never acquires decoded definition or Property counts. Failed structured sources have null counts,
not a healthy zero inventory. The existing normal-refusal and explicit diagnostic-generation
policy applies; failure never replaces a prior complete media index.

`p4kx_shader::Limits::validate_configuration` checks supported configuration without inspecting
source content. `read_with` checks its original checkpoint and source-byte admission first.
A failure's `limit` is the reported reader bound; an unsupported nesting configuration reports
the supported maximum. Source-size refusal, configuration-only refusal and actual parser failure
have distinct field-origin profiles. Filename and directory-size values retain their metadata
origins; parser-derived diagnostics and byte ranges retain consumed payload contributions.
Configured bounds, literal controls and unavailable counts have complete empty contributor sets.
Retained raw Source and Record origins independently preserve their actual encrypted witnesses.

## Schema and integration scope

`shader_payload_schema()` validates the precise tagged payload union. `shader_record_schema()`
adds payload/status/issue conditions only for `kind: shader` and the five built-in namespaces:
`shader-definition`, `shader-property`, `shader-declaration`, `shader-cache-inventory` and
`shader-definition-failure`. Custom provider namespaces retain the generic record data contract.
Nullable declaration fields remain explicitly required; interpreted words are schema-bounded
through the full unsigned64 decimal range.

`builtin_record_schema()` supplies the fallible common factory used by `media schema --record`.
It currently binds the shader payload contract and rejects conflicting generated definition names.
The existing `record_schema()` remains the generic library envelope. The checked-in
[built-in record schema](../schemas/media-builtin-record.schema.json) describes the CLI output;
the [generic record schema](../schemas/media-record.schema.json) remains available independently.

## Material declaration links

With `--kind material --kind shader`, material libraries emit `material-shader-reference` batches.
The ordinary layer/texture/palette `references` array remains unchanged. Each new reference retains
its original material index, authored attribute position, optional selected-token position, role,
original shader name and original token. Absent Shader attributes and engine-default placeholder
slots create no invented empty declaration. Repeated or case-distinct Shader attributes are kept;
without one unique authored shader scope, selected flags are explicitly unavailable.

The export-local lookup uses the same bounded traversal and container-opening rules as projection.
It inventories physical `.ext` names across the root and nested containers, independently of
encounter order. Matching ignores ASCII case while preserving every original spelling and physical
candidate. Duplicate filenames, case collisions and identical inner paths in separate containers
stay ambiguous. Explicitly excluded targets are never read; an unscanned container or damaged
inventory prevents a Missing or unique-resolution claim. Targets outside selection or an unrequested
shader kind stay NotSelected and are not automatically added to the export.

A unique selected definition is read through the existing verified private-input path and the same
shader parser. Target identity uses the opaque observed Archive input plus the existing serialized
Root/container/Entry identity. Successfully read direct Property Name occurrences point to the
exported `shader-property` namespace and original declaration ordinal, including repeated names.
Quoted simple names are matched without their delimiters; unsupported expressions and escapes
remain raw in the shader output and make lookup availability explicit. Local Property matching does
not evaluate masks or execute dependencies. Missing local flags under UsesCommonGlobalFlags remain
unavailable rather than assuming undocumented global inheritance.

Every actual lookup read carries an `InputReceipt` with complete measured decoded digest, physical
source and exact use location. Its `reference-lookup` omission means these private lookup bytes are
not published by that read. A separately selected source still receives its ordinary raw artifact;
this receipt does not assert another retained source callback or grant publication authority.
Repeated references reuse one immutable parsed definition within the export. Inventory work,
container bytes, input reads, query/candidate counts and record size share their existing bounds.

Shader and material provider versions are 1.1.0 within the media 1.0 envelope. Shader-only output
keeps the same source/Property payloads and requires neither material parsing nor lookup. The
new material field is additive; existing MTL, Blender package, catalog and rendering contracts
are unchanged. No shader compilation/execution, numeric GenMask interpretation, `.dpsi`/`.fx`
grammar or in-game appearance equivalence is implied.

## Verification boundary

Synthetic production-export fixtures cover nested and duplicate/case-colliding definitions,
repeated Property names, selected/unselected/excluded scopes, missing and unsupported names,
source receipts, limits, schema and unchanged selected-source accounting. The ignored
`shader_accounting::nested_shader_sources_properties_and_raw_artifacts_reconcile` test discovers a
bounded available nested `.pak` shader family without private hardcoded names and reconciles actual
source/Property/JSONL/raw/index accounting. Its `.socpak` exclusion is explicit test scope; production
supports both container types. An empty or incompletely enumerated family fails verification.
A diagnosed parse failure is reconciled only when independently rereading the retained raw bytes
reproduces the exact error and byte span and the diagnostic generation remains incomplete.
Installed LIVE/PTU execution and any applicable native/consumer acceptance must be recorded
separately; adding or compiling the ignored test does not prove those runs occurred.
