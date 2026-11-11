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

## Physical loose WEM inventory

Every opened Archive directory has an `audio-loose-wem-inventory` closure. Its
`data.payload.observed_candidates` is an unsigned decimal string. Each recognized numeric
loose WEM path emits one `audio-loose-wem-candidate` row in directory order. The row is
keyed by the owning `source.root` and `source.identity`, and by the numeric
`data.payload.media_id`. Its only subresource index is the candidate's physical leaf
Entry index. The `data.payload.entry` identifies that leaf. For a nested scope, the
owning source's containing Entries followed by its own Entry give the leaf's full
container chain. Repeated paths and Short IDs keep separate physical rows.

`placement` records a neutral path, the original language folder, or unavailable
placement for a lossy path. `selection` records the candidate's export selection
separately. `payload_access` is `key-required` when an encrypted candidate has no
configured key in the observed scope, or `not-inspected`. Candidate rows never open
the target payload. Selected WEM metadata and the source manifest carry any retained
raw artifact. These directory rows describe the inventory independently of a
requested language or a fallback decision.

A scope with `candidates_complete: true` and no matching Short ID establishes absence
in that owning Archive. A recovered directory emits a partial closure in diagnostic
output. An unopened directory has no closure. Excluding a container leaves its scope
unavailable. Source, work and output budgets still apply to the whole generation.
A budget failure leaves the last complete generation available.

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

## Optional WAV and Ogg output

By default, the Audio provider retains the original bytes and writes the metadata above.
Request 16-bit PCM WAV output with the following command.

    p4kx media export ARCHIVE --out DIR --kind audio --audio-format wav

The provider writes 16-bit little-endian WAV from PCM, IMA ADPCM, PTADPCM and Opus in the first
selected DATA chunk. It handles standalone WEMs and complete WEMs in an unambiguously resolved
BNK DATA range. The WAV preserves the channel count, playback rate and applicable loop. PCM
uses complete frames. IMA ADPCM also decodes a final block whose channel sub-blocks fit the
original bytes. PTADPCM uses complete blocks. Unused tail bytes and extra chunks stay in the
raw artifact. Opus WAV uses the original Wwise seek-size table, Switch packet framing or embedded
Ogg pages. It applies the pre-skip, channel order, output gain and checked sample trim.

Request deterministic Ogg artifacts from Wwise Vorbis or Opus with the following command.

    p4kx media export ARCHIVE --out DIR --kind audio --audio-format ogg

Vorbis uses the original Wwise configuration, setup packet and audio packets. Opus supports the
Wwise seek-size table, Switch packet framing and embedded Ogg pages. The first `data` chunk bounds
each packet walk. Both outputs preserve the channel count, playback rate, checked frame count and
applicable loop. Opus applies the original pre-skip and trims at the checked final granule.
Standalone conversion and the Media provider share each codec's packet interpretation and Ogg
writer.

The `audio-wem-conversion` and `audio-bank-wem-conversion` namespaces contain one
`wem-conversion` record per eligible occurrence. The `written` outcome records the
requested and actual formats, source codec, decoded dimensions and loop, original
half-open range of consumed PCM frames, coded ADPCM blocks or selected compressed packet data, and verified
artifact path, length and SHA-256. Ogg Vorbis rows add `vorbis_inputs` with the original
average-byte-rate word, configuration and size-prefixed setup packet ranges. Opus source rows add
`opus_inputs` with the selected format prefix, checked DATA range and any selected seek words or
embedded header pages. The emitted Opus source range can end before the checked range after an
authored sample trim. Artifact origins retain the selected inputs and ranges independently. Embedded
ranges are absolute within the retained bank, with the DIDX section and triplet as
subresource indices. Equal output bytes of the same format can share one file, while the conversion rows and
artifact-manifest occurrences retain their own physical source uses and encryption markers.

A complete WEM outside the requested format's codec coverage receives an `unsupported-codec` conversion
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
the actual codec and count inputs, including the first `data` length for PCM and supported ADPCM.
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
