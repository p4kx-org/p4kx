# Player character source

`p4kx character source <archive> --body <male|female> --out <file>` writes a
[p4kx-player-source v1 document](../schemas/player-source.schema.json). The explicit body
selects the corresponding conventional default CHF. Each item port retains its original
parent, sibling ordinal, CRC and item GUID. The joined class retains the physical DataForge
record ordinal and original GUID. Duplicate class matches are preserved, and empty slots,
missing targets and unreadable sources have distinct states. Each non-root CHF port also
links its original CRC to the matching port definitions on its immediate parent item class.
The link retains each candidate's physical component and port ordinal. The original flags,
attachment type and ordered helper alternatives retain their tags and local offsets. The authored
`noRotation` constraint is available as a source value.

The export inventories every XML entry under the archive's player default-loadout directory
in archive order. Each `Items` group or `Item` has its original XML coordinate and an
optional index of its direct parent. An empty `itemName` and an omitted attribute remain
distinct. The `documents` table retains original attributes and element order, including
sections outside the typed item tree. Consumers can select loadouts without assuming
which one is used at runtime. A nested XML item's port definition is looked up on the class
explicitly named by its immediate parent item. Root items and items beneath an empty or omitted
parent selector report that owner state. A duplicate class or port name retains its candidates.

`motion` identifies the DataForge Player actor and its original animated-character component
slots. `actionController` and `animDatabase3P` select original archive XML documents. The
document indexes reference controller scopes, fragment alternatives and ADB subdatabase
declarations in their original order. When both XML roots are readable, the component includes
`authoredOptions` for direct root `FragmentList` options in ADB order, with separate original
`Tags` and `FragTags` values. The controller `Fragments@filename` and ADB `FragDef` must
identify one physically observed shared definition document before an option links to same-name
controller fragment candidates. Each unambiguous controller fragment retains its original
scope tokens and their candidate XML coordinates. An empty candidate list is an observed miss.
The original documents retain the nested ADB selectors. Physical clips and gameplay variant
selection require additional source context.

Source-backed values expose `containsEncryptedSource`, `sourceSetComplete` and
`sourceUsesRefs`. A join preserves both the original selector and observed target source.
Raw XML values use their document's physical identity and their original local position.
