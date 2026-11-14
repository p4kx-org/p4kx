# The MCP audio excerpt is a bounded WAV and the only tool that returns audio

Decided for issue #287, on top of ADR-0038 (the legal notes of the MCP tools) and ADR-0018 (the
audio index).

## Decision

The MCP server answers questions about audio with metadata and paths: `search_audio_triggers`,
`get_entity_sounds`, `resolve_audio_trigger`, `list_soundbank_wems` and `get_wem_info` return
trigger and event names, IDs, soundbank and Entry paths, codec, channels, sample rate and
duration, and never a byte of audio.

`get_audio_excerpt` is the one exception, because the issue asks for it and a model cannot judge a
sound it cannot hear. It is bounded so that it stays an excerpt and not a way to move the sound
out of the Archive:

- at most 10 seconds (5 by default), and the WAV is cut where it would exceed 1 MiB; `truncated`
  is true whenever the excerpt is shorter than asked for, whether the 10-second limit, the size
  cap or the end of the sound cut it, and `max_bytes` states the cap;
- the audio is a 16-bit PCM WAV in an `audio/wav` content block next to the structured metadata;
  the structured content never holds audio bytes, so the output schema carries none;
- the decoder writes into a window that drops the frames before `start_seconds`, keeps the
  window and stops the decoder once it is full (when the header states the frame count), so a
  long sound gives a short excerpt and the WAV is never held whole; an excerpt that would reach
  beyond the first 64 MiB of decoded audio is refused;
- at most two audio reads run at once (the model permit);
- plug-in media and the prefetched start of a streamed sound cannot be decoded and are refused;
  a WEM ID with a loose Entry is read from the Entry, not from the prefetched start;
- the description says that the audio is game media and must not be redistributed, as every tool
  that returns Archive content does.

## Consequences

The audio index (every ATL definition and soundbank) is built once per open Archive and kept with
it, by the first audio call. `ArchiveAudio::with_index` lets a service user keep the index itself.
The calls that read a WEM or a soundbank (`list_soundbank_wems`, `get_wem_info`,
`get_audio_excerpt`) take a model permit inside the tool, after the audio index is built or
fetched, so a minutes-long index build never holds a permit. The trigger and Entity tools only
read the index and take none.

`get_wem_info` pages its `locations` with a cursor like the other lists and reports the language
folder of a WEM selected by `path`. A cursor beyond the listing is rejected as soon as the size
of the listing is known (the places of the WEM, the triggers of the Entity), before any WEM is
read or any item is built.

Candidate search follows the complete indexed list in its existing order: loose Entries first,
then embedded locations in soundbank path order. Metadata selects the first whole WEM that
parses; an excerpt selects the first candidate that decodes the requested window. Paging the
location list does not bound that search. Only after all indexed candidates fail does metadata
retain its first observed fallback shape, or the excerpt return its first actual error ahead of
a prefetch-only result. Explicit Entry paths still select that one Entry.

The calls keep one candidate source at a time and retain their existing source-byte, decode
window and response limits. Total candidate work can grow with the indexed list and the bytes
read or decoded in failed attempts; this is not a bounded-time guarantee. The current blocking
tool runner has no request-cancellation token to pass into these loops. No candidate-count
prefix substitutes for a complete negative result. A prefetch-only result describes these indexed locations.

Memory per call, stated honestly: the source is read whole before it is parsed. A loose WEM up to
512 MiB (the cap of `Sound`), a soundbank up to 32 MiB (the cap of the audio index; a larger
Entry is refused by `list_soundbank_wems` and `get_wem_info`), plus the 1 MiB window and the
base64 text of the WAV, 1.4 MiB, which is more than the 256 KiB cap of the DataForge tools.
With two permits that is at most about 1 GiB for two loose WEMs of the largest size. Every result is held to the existing 256 KiB response cap, including its envelope.
Retained text is complete. Each actual output occurrence is admitted before copying through the
existing Budget, with one budget for the envelope and one for each materialized item and its
nested values; exact JSON admission remains final. Display text uses the bounded formatter.
An oversized later item retains the complete prefix and a cursor that retries that item;
an oversized first item is refused. Other rendering errors remain fatal.

Event lists retain every admitted original event, including every actual search decision.
The remaining previews retain up to 10 soundbanks per event, 25 broken links and 20 issues
with their stated totals. These limits need separate completeness/retrieval
assessment; totals alone do not justify omitted nested entries. Existing input byte caps remain
independent: a complete returned trigger above 256 bytes is explicitly refused if passed to
`resolve_audio_trigger`. Names within that cap and returned Entity/Entry paths within their
respective input caps remain reusable without clipping.

`search_audio_triggers` loads DataForge and reads Entity Records only when `entities` is above
zero and the selected page has actual hits. Its existing owned selected-name bridge also carries
the actual match and unmatched decisions through the worker boundary with the same immutable
AudioIndex. This selection work is not claimed to be bounded by the response budget. Actual
DTO and User-header copies plus JSON/page admission run lazily inside that worker. DataForge
load failure remains an explicit `entities_skipped` outcome; a rendering/budget failure does not
become a skipped lookup. No matcher, parser, registry or source-input authority is reconstructed.
