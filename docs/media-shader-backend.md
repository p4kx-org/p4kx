# Shader source projection

`p4kx media export ARCHIVE --out DIR --kind shader` exports source extension declarations and
opaque compiled-cache inventory. The default registry includes the permanent Archive provider and
`ShaderProvider`; the library also accepts an explicit `ProviderRegistry` through `export_with`.
Material-to-definition lookup remains separate work.

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

Material tokens and declaration links require the planned bounded complete-inventory lookup.
This backend does not execute shaders, decode compiled caches, add `.dpsi`/`.fx` grammars, change
MTL/Blender/catalog output, or claim appearance parity. Real-source accounting and the remaining
issue #1109 acceptance work remain separate.
