# Container occurrence keys record external Child steps

The complete PTU catalog contained distinct container occurrences with the same system root and
Child ordinals. An authored Child can contain inline descendants and open a separate descriptor.
Both documents can contribute a Child with the same ordinal, with different physical sources,
parents and transforms.

Schema v3 extends `ContainerInstanceKey` with `externalChildStepIndices`. It records the ascending
zero-based positions in `childNodeIndices` where the walk reads a Child from an external
descriptor entered at the preceding step. The Child that declares an external link retains its
original document identity. An empty list marks an inline-only route. This keeps the identity
independent of file spellings and source-use IDs and adds entries only for actual document
transitions. The existing root index and Child ordinals still support grouping and navigation.

Resolved instances, unresolved links, location and jump placements and the Mining SDK use the same
complete key for occurrence and parent joins. The SDK keeps the ordinal-only selector for callers
that want every matching route. The exact selector also accepts the external step positions.
Regenerate development catalogs containing the older key before joining their placement rows.
