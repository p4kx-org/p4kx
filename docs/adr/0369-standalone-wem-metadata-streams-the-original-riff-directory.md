# Standalone WEM metadata streams the original RIFF directory

Media Audio reads a summary from the original prefix through the first `data` and uses a
separate checked traversal for the full RIFF directory. Ordered chunk ranges reference the
verified retained Entry, which preserves duplicate and unknown chunks without storing a copy
of each body. Structural directory failures use diagnostic publication so that consumers can
recover the valid prefix and raw bytes with an explicit incomplete status.

[Audio metadata contract](../media-audio-metadata.md).
