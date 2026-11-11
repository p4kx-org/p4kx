# WEM metadata in the media export

`p4kx media export ARCHIVE --out DIR --kind audio --filter '*.wem'` projects selected
standalone WEM Entries, including Entries inside selected traversal scopes. Extension matching
ignores ASCII case. Each physical occurrence has its own records, even when duplicate names
or equal decoded bytes share one raw artifact. `source.identity` and `source.raw` identify the
original Entry and the exact verified decoded bytes retained in this generation.

The `audio.jsonl` records use the Media 1.0 envelope. Built-in namespaces have typed payload
constraints in `p4kx media schema --record`.

| Namespace | `data.record_type` | Contents |
| --- | --- | --- |
| `audio-wem` | `wem` | One RIFF/RIFX summary at empty subresource indices |
| `audio-wem-chunk` | `wem-chunk` | One original RIFF chunk at its zero-based ordinal |
| `audio-wem-cue` | `wem-cue` | Original declared cue-point count at the parent chunk ordinal |
| `audio-wem-cue-point` | `wem-cue-point` | One cue point at the parent chunk and original point ordinals |
| `audio-wem-cue-tail` | `wem-cue-tail` | Residual cue range after all declared points |
| `audio-wem-list` | `wem-list` | Original LIST type at the parent chunk ordinal |
| `audio-wem-list-item` | `wem-list-item` | Immediate child at the parent chunk and original child ordinals |
| `audio-wem-annotation-failure` | `wem-annotation-failure` | Failed cue or LIST structure at its parent chunk ordinal |
| `audio-wem-opaque` | `wem-opaque` | One raw PLUG or MIDI occurrence with its recognized class |
| `audio-wem-failure` | `wem-failure` | Structural failure at the RIFF header, summary or directory phase |

## Summary and original directory

The summary uses the last `fmt `, `vorb` and `smpl` before the first `data` as applicable
to the codec. It retains the original format tag, byte order, channel count, block alignment,
bits per sample, optional original channel layout, and derived speaker mask. Both declared
and playback rates are in Hz. `sample_count` counts frames with one sample per channel.
Frame counts and loop positions use canonical unsigned decimal strings. A loop spans
`start` through the frame before `end`. An unavailable count, duration or loop is `null`.
Duration has whole `seconds` in decimal text and a numeric `nanoseconds` remainder.

A complete directory includes duplicate FourCCs, unknown tags and chunks after the first
`data`. Every `tag` contains the original four bytes as unsigned byte values. Each
`header_range` and `body_range` is half-open and relative to byte zero of `source.raw`.
A separate `padding_range` records the padding admitted by the scanner, or `null` when no
padding was traversed. `riff_end_exclusive` marks the declared end and `trailing_length`
counts any remaining retained bytes beyond it. Read any range from the exact raw artifact
using the recorded byte boundaries.

PLUG and MIDI occurrences have a class and a raw artifact.
[Soundbank metadata](media-bank-metadata.md) describes the original BNK and embedded media ranges.

## Cue points and LIST annotations

Each cue point retains the original ID, play position, target FourCC, chunk start, block start
and sample offset. Duplicate IDs retain their original positions. A completed cue scan carries
an exact residual range, including an empty range. A malformed point retains the readable prefix
and declared count in the diagnostic generation.

LIST rows retain the original type bytes and each immediate child header, body and padding range.
An associated-data list (`adtl`) projects `labl` and `note` with the original cue ID, full raw text
range, text range before the first NUL, nullable terminator offset and UTF-8 validation result.
`utf8_text` contains the accepted UTF-8 interpretation, including original whitespace, or `null`
for invalid bytes. `utf8_input_range` describes the actual validation prefix. Terminator selection
uses its own original byte range, which can continue after UTF-8 validation encounters an error.

An `ltxt` row retains the original cue ID, sample length, purpose, country, language, dialect,
code page and exact text byte range. Label and note UTF-8 validity describes their bytes.
The original code-page declaration belongs to the text region. Unknown LIST types and child
tags preserve their body ranges as opaque content. A short recognized child retains its exact
physical range and a partial status. Diagnostic output can still retain later readable children.

## WEMs embedded in soundbanks

For every DIDX occurrence with a complete RIFF or RIFX range in one resolved DATA section,
the provider emits the same summary, chunk, cue and LIST fields under `audio-bank-wem` namespaces.
For example, `audio-bank-wem-chunk` has indices `[DIDX section, DIDX triplet, RIFF chunk]`.
Other embedded rows prefix their standalone index path with the same two bank ordinals.
Duplicate media IDs retain separate rows. Prefetched starts, MIDI, plug-in media and unresolved
DATA links retain their original DIDX rows and ranges where available.

Byte ranges, the RIFF end and LIST terminators in these rows are absolute offsets into the
retained BNK Entry. Counts and trailing lengths describe the individual WEM. In source-use
receipts, WEM coordinates follow the two DIDX ordinals and keep their original WEM-local
interpretation. Separate receipts identify the actual triplet, DATA extent and RIFF selection.

## Optional PCM WAV output

By default, the Audio provider retains the original bytes and writes the metadata above.
Request PCM WAV conversion with the following command.

    p4kx media export ARCHIVE --out DIR --kind audio --audio-format wav

The provider writes 16-bit little-endian WAV from complete frames in the first selected
PCM DATA chunk. It handles standalone WEMs and complete WEMs in an unambiguously resolved
BNK DATA range. The WAV preserves the channel count, playback rate and applicable loop.
Partial input frames and extra chunks stay in the raw artifact.

The `audio-wem-conversion` and `audio-bank-wem-conversion` namespaces contain one
`wem-conversion` record per eligible occurrence. The `written` outcome records the
requested and actual formats, source codec, decoded dimensions and loop, original
half-open frame byte range, and verified artifact path, length and SHA-256. Embedded
ranges are absolute within the retained bank, with the DIDX section and triplet as
subresource indices. Equal WAV bytes can share one file, while the conversion rows and
artifact-manifest occurrences retain their own physical source uses and encryption markers.

A complete WEM whose codec cannot produce WAV receives an `unsupported-codec` conversion
outcome. Normal export reports projection loss. A diagnostic generation retains the
original source and the failure. Prefetch and opaque media retain their raw metadata.

## Read and publication boundaries

The seekable metadata reader uses the retained Entry and the generation's input-byte, metadata
work and cancellation checks. The directory scan streams one chunk record at a time.
Encoded audio and unknown chunk bodies remain recoverable from the raw artifact.

A malformed directory suffix can leave a readable summary and valid prefix chunks.
Normal export reports Audio projection loss. With `--diagnostic`, the incomplete generation
retains the valid records, exact raw bytes and the failure row, while the root index continues to identify the last complete generation. The failure's subresource index identifies its phase
(`0` for RIFF header, `1` for summary, `2` for directory). Physical read, cancellation and
budget failures still follow the shared generation error rules.

Each record's immediate origin fields are in the generation's field-origin companion.
Format values identify the selected original `fmt ` body. Derived frame counts identify
the actual codec and count inputs, including the first `data` length for PCM.
Chunk positions and tags refer to the original traversed positions. Directory metadata
such as `source.identity.entry.path` has independent provenance from the WEM payload.
A malformed read may identify the physical input without a complete source-coordinate set.
The marker follows the actual input scopes and container ancestry.

## Installed Archive verification

With `P4KX_DATA_P4K` and the stored Archive key configured, the opt-in Media test samples one
standalone WEM and one bank with HIRC objects and DIDX media from each configured Archive. It
compares the export with the slice readers and original DIDX bytes, validates the built-in record
schema and checks the retained bytes and physical identities. Each sampled Entry is at most
8 MiB. Results report failures without exposing private source paths or values.

    cargo test --release --locked -p p4kx-media --test real_archive original_audio_directories_and_retained_ranges_reconcile -- --ignored
