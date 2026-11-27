# Direct mining and resource queries retain canonical membership dependencies

Mineable elements, mineable compositions and ResourceType definitions have selective SDK entry
points that reuse their complete canonical source-owned readers. A result retains the original
source cells, numeric widths and bits, null and unresolved-reference states, physical Record
identity, selected text rows and immutable source inventory. The ResourceType view exposes the
same nested density, cargo, crafting-quality and original-value fields that the Catalog serializes;
it is not the mining Resource summary. No JSON bridge or replacement DTO is involved.

All, GUID and physical Record-index selections use the ordinary Catalog admission policy. GUID
selection retains every eligible physical duplicate; RecordIndex disambiguates them. Rows use
canonical textual GUID order with the physical index as a tie-breaker. Reference resolution keeps
the existing authored target policy rather than introducing a new ambiguity state.

ResourceType.groupGuids requires a complete ResourceTypeGroup-family survey, including groups
outside the selected ResourceType. The shared ResourceType reader requests this dependency only
when it interprets that field. Its first demand reads the necessary family once; subsequent selected
resources reuse the same membership index. This dependency does not materialize bodies from other
families. Empty, excluded or unreadable selected resource owners request no group bodies.
Elements and compositions never request this dependency and do not open world placements.

Owner coverage and demanded membership-family coverage remain separate. Unclassifiable surveyed
headers, unreadable group bodies and uninterpretable membership slots make group knowledge
incomplete; they are not evidence of absence. The canonical sourced membership cell retains that
incompleteness. A successful callback and outer Result do not establish complete coverage. Consumers
must examine both coverage results and the returned source states before publishing their caches.

A parsed query session reuses the decoded DataForge and its indexes. Only requested localization
payloads needed by successfully read selected owners are opened. Text identities and source-use
references resolve through the same result's text and source inventories. Borrowed rows remain
inside the callback; consumers publish owned results only after the outer query succeeds. Existing
Schema v3 field meanings and wire values remain unchanged. These three endpoints do not complete
the independent world, placement, harvest-location, refining, item or blueprint SDK domains.
