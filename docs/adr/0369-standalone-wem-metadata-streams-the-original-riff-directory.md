# Standalone WEM metadata streams the original RIFF directory

Media Audio reads a summary from the original prefix through the first `data` and uses a
separate checked traversal for the full RIFF directory. Ordered chunk ranges reference the
verified retained Entry, which preserves duplicate and unknown chunks without storing a copy
of each body. Structural directory failures use diagnostic publication so that consumers can
recover the valid prefix and raw bytes with an explicit incomplete status.

An explicit WAV request streams complete 16-bit PCM frames from the selected retained WEM
range, including complete embedded WEMs. The seekable writer and the slice writer share the
canonical WAV header and byte-order rules. Each converted artifact occurrence binds the
actual frame range and header inputs. The converted file can deduplicate by digest while
source identity and encryption provenance stay attached to the occurrence. The original
WEM and its metadata remain available. Conversion failures follow the common diagnostic
and generation commit rules.

[Audio metadata contract](../media-audio-metadata.md).
