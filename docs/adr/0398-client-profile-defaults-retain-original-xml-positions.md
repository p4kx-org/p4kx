# Client profile defaults retain original XML positions

## Context

Archive client profiles contain independently readable general and VR attribute files.
Some attributes associate a setting with a CVar. The same input family uses text XML
and CryXml. The ActionMap has different source members and exposes authored bindings.

## Decision

Use one verified client-profile reader for the CLI, SDK and Catalog v3. Keep each exact
physical file's selection and read outcome. Preserve element order and parents, authored
attribute and namespace positions, raw value spelling, and unknown declared values.
A repeated name remains an independent occurrence. All values cite their actual
payload or directory evidence.

A client profile export describes authored defaults. Consumer projections may interpret a
setting once another source establishes its unit or symbolic values.

## SDK selection

Use the profile role as the stable lookup identity for
`/configurations/clientProfileSettings/documents`. Each result preserves
the matched physical Entry ordinals and source references within its Archive.
The directory establishes missing, filtered and ambiguous outcomes. A scoped
query only opens the selected profile payload and returns its own source inventory
after checking Archive freshness. It does not load DataForge or localization.
