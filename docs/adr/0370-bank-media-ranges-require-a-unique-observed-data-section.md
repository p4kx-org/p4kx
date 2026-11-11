# Bank media ranges require a unique observed DATA section

The first streaming pass retains every physical section and establishes DATA uniqueness. The
second uses that evidence to project each original DIDX triplet and HIRC object with fixed
working memory. Ambiguous or incomplete DATA selection retains the original declaration
with explicit resolution status, so repeated short IDs and duplicate sections remain traceable.

The selected DATA section lends a checked, media-relative read window for each complete WEM.
The shared RIFF reader projects it without materializing the bank or adjacent media. Exported
byte ranges remain absolute in the retained Entry. Embedded subresource identity includes the
original DIDX section and triplet positions. Each value carries the actual DATA selection and
WEM read evidence. Prefetched and opaque ranges remain represented by the DIDX metadata.

A uniquely observed bank header selects the HIRC layout. Action, Event, Sound and container interpretation use
the same versioned field readers as the existing slice decoder. Media emits physical envelopes
and typed records, preserving original order, readable prefixes and residual bytes. An unknown
layout retains the raw envelope and records its coverage explicitly.

[Soundbank metadata contract](../media-bank-metadata.md).
