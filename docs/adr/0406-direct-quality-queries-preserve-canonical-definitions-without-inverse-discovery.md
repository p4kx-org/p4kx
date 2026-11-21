# Direct quality queries preserve canonical definitions without inverse discovery

The SDK exposes complete canonical distribution/location-override and quantization definitions
without constructing the full Catalog or computing the inverse ResourceType projection. These
are independently useful datasets: an authored quantizer can be read even when it is ineligible
for inverse lookup, its reference target is missing, or another same-GUID physical declaration
wins the existing inverse policy. The inverse has a separate opt-in SDK endpoint.

A narrow `catalog_queries::quality` module supplies free Archive/decoded-Entry functions and
session methods. All, GUID and RecordIndex selection use existing canonical source rules and
leftover policy. Both distribution owner types participate. Equal GUIDs retain every admitted
physical owner, ordered by textual GUID and physical index; authored arrays retain their original
slots and repetitions. References keep the canonical target-header policy without opening
referenced bodies. No new numeric, reference or eligibility interpretation is introduced.

The reader calls the existing canonical distribution/quantization source readers with original
Record admission, field capture and unchanged numeric receipts. Immutable typed getters expose
every serialized nested value, and the result owns its exact source/use inventory. Original integer
widths, Int64 digits, IEEE bits, null and unavailable states remain intact. Exact Single recovery
uses raw bits before binary64 promotion. The finite JSON convenience value continues to use the
normal shortest Single decimal representation; this adds no wire or formula change.

Only selected owner bodies are materialized. No ResourceType, sibling quality, locale, world or
whole-crafting body census is required, and no fixed materialization quota is introduced. Header,
body and field projection coverage remain separate from source declaration availability and from
successful query completion. Consumers publish owned cache updates after outer success and the
necessary coverage checks. The existing query session retains cancellation, Archive-change and
physical Entry-origin checks.
