# ADR-0374. ATL declarations retain physical XML identity and authored edges

## Status

Accepted.

## Decision

AudioControl exports a physical XML source inventory. Its source is the verified decoded ATL
Entry. Text XML uses original half-open byte ranges. CryXml uses original node and attribute
table coordinates. The raw Entry remains authoritative for lexical details and repaired strings.

Element, content and declaration occurrences stream as separate records. Typed family and
field records join to these originals. A field's decoded scalar can be inspected independently
of its original literal. File and element occurrences own names. Repeated names and
declarations keep their original positions.

The owning Archive directory bounds named preload and RTPC searches. A platform group belongs
to its physical preload request. The exporter records every authored edge directly and never
traverses the graph as playback. Unknown inventory keeps the result unavailable. Explicit file
paths use the exact directory index and separate identity, selection and key state.

An ATL Wwise Event name can request the shared Audio HIRC inventory when Audio is selected.
That lookup uses checked retained soundbank metadata once per owning directory. The reference
retains its original ATL proof and candidate rows retain their own physical source proof.
Runtime bank, language and playback choices belong to their existing consumers.

## Consequences

An AudioControl-only generation can export every selected ATL declaration with no WEM or
BNK semantic decoding. Duplicate candidates and declaration order remain reproducible.
Source and runtime metadata, including event duration bounds, can be interpreted separately.
