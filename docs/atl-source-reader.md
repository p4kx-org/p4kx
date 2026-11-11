# ATL source reader

ATL discovery matches XML Entries below `Data\Libs\GameAudio\` with ASCII-case-insensitive
path and extension matching. Both slash spellings are recognized. The format reader borrows
admitted bytes and uses the original CryXml decoder for binary documents. Element traversal
borrows only direct children, in the order given by each parser.

Text element and attribute coordinates are half-open byte ranges in the original input, including
the offset of a leading UTF-8 BOM. The full attribute span preserves its original QName,
whitespace, quote spelling and value literal. Parsed text names expose local spelling and namespace
URIs. The Assembly lookup takes the first local-name match, including qualified attributes.
The authored syntax view exposes exact QNames, ordered attributes and namespace declarations,
decoded values, original value lexemes and original declaration coordinates. Text, comments,
processing instructions and child elements keep their parser order and original byte extents.
An exact text extent may include adjacent lexical pieces such as a CDATA section.
The document view includes parser-observed prolog and epilog content.

CryXml coordinates identify actual node and attribute table indices. Binary names retain
the binary decoder's spelling and duplicate attributes retain separate table positions. Each
CryXml node's text slot is separate from the order of its children. `SourceDocument::is_lossy`
reports repaired CryXml string-table bytes.

## Bounds

The legacy Assembly audio index admits up to 8 MiB per ATL Entry. It applies its 1,048,576-node
text parser budget and the existing 128-open-element preflight to text only. It also retains
its independent 262,144-trigger index capacity. The index keeps its existing trigger and event
selection and reports the first definition that exceeds the trigger capacity.

The shared parser accepts an optional caller-selected text node limit and open-element preflight
limit. With the default options, the text tokenizer tracks open elements iteratively and admits
the pinned parser's representable node, attribute and namespace ranges. The parser returns errors
when its index widths are exhausted. Its text node and attribute indices and tokenizer depth use
32 bits. Distinct namespace table positions use 16 bits. The text parser keeps its default DTD
policy and supplies no external entity resolver. Whole attribute ranges remain exact when the
text parser's narrower internal QName or separator-length fields saturate.

The CryXml decoder checks table positions and lengths against the admitted bytes and validates
parent links and attribute references. It derives child traversal from those parent links. The
shared API adds no fixed binary count or depth limit. Export callers supply their byte and work
admission and poll cancellation between bounded operations. A subsequent audio-control export
must account for malformed sources, raw declarations and provenance at its normal writer boundary.
