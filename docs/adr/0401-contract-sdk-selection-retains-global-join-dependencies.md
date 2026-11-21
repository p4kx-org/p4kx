# Contract SDK selection retains global join dependencies

Selecting a physical Generator owner does not change the canonical meaning of its contract
rows. The SDK resolves needed mission dependencies, intro-grant candidates, logical duplicate
candidates and property-set aliases outside that root selection, while retaining physical owner
identity and the original source evidence of each decision. It validates a related Generator with
the shared complete Contract transaction before using its grants or rows; one failing entry makes
that Generator's derived contribution unavailable.

A source-only reverse survey identifies related Generator owners without projecting unrelated
sources. Known failed surveys are explicit coverage gaps and make affected grant joins incomplete.
The canonical duplicate and property-set decisions run before output is restricted to selected
owners. A losing selected candidate is reported with its global winning owner. Consumer code can
therefore distinguish source absence, an incomplete dependency survey and a shadowed logical row.

The complete Catalog producer uses the same entry enumeration, Contract transaction and finalizer.
Its existing Filter semantics stay unchanged. Dependency-only validation has its own text registry;
its unused titles cannot enter the selected result's text tables. Published dependency tables
register their own referenced text keys through the selected query's registry. Required owner trees are reused by the dependency index without an extra payload decode or a
self reference. A logical-key survey drops unrelated trees, retains compact reverse summaries,
and materializes only subsequently identified outside dependencies again.

The public SDK exposes the same canonical typed rows and all nested exported values. It does not
serialize a complete Catalog or require consumers to inspect JSON to traverse a property, source
arena, spawn description, reward or title token. This preserves useful narrow reads on machines
that cannot afford building the complete Catalog.

The additional native `ContractKey` selects one `(contractGuid, subContractGuid)` pair without
changing the physical-owner API. All exact physical candidates are validated before canonical
selection; `None` identifies the parent only. Its source survey collects identities before deeper
row dependencies so a later failed guard does not erase an already observed candidate. Reverse
intro and property-alias dependencies derive from that exact row's requirements and effective set
links. The shared finalizer resolves conflicts and duplicate ordering before exact-key filtering;
its output contains only the matching row and title. Caller keys are not serialized source facts.

Forward preparation follows actual canonical typed lookups, not a recursive scan of original
References. The private prepared scope distinguishes a nominal family/GUID from a physical mission
item. Each demanded group includes all eligible original candidates before the canonical selector
can report absence or choose a duplicate. A typed internal demand is intercepted before source
failure classification; probes discard their partial derived contribution and their text registry.
The final attempt alone supplies rows and grants. Even a source-failed final owner keeps its partial
text registrations private. Published place names use the exact successful-attempt lookup trace.

The query builds its GUID header index once, caches required bodies and incrementally extends the
same canonical MissionIndex. Prepared keys distinguish proven, absent and failed source outcomes.
A retry must make progress by preparing a new key, with repeated prepared requests reported as an
accounting invariant error. This preserves cancellation and genuine source failures without a retry
quota, source fallback for unprepared inputs or an unrelated full-domain projector. The complete
producer retains its ordinary prepared-input behavior.
