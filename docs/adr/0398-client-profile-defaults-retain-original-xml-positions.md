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
