# Player character source

`p4kx character source <archive> --body <male|female> --out <file>` writes a
[p4kx-player-source v1 document](../schemas/player-source.schema.json). The explicit body
selects the corresponding conventional default CHF. Each item port retains its original
parent, sibling ordinal, CRC and item GUID. The joined class retains the physical DataForge
record ordinal and original GUID. Duplicate class matches are preserved, and empty slots,
missing targets and unreadable sources have distinct states.

The export inventories every XML entry under the archive's player default-loadout directory
in archive order. Each `Items` group or `Item` has its original XML coordinate and an
optional index of its direct parent. An empty `itemName` and an omitted attribute remain
distinct. The `documents` table retains original attributes and element order, including
sections outside the typed item tree. Consumers can select loadouts without assuming
which one is used at runtime.

`motion` identifies the DataForge Player actor and its original animated-character component
slots. `actionController` and `animDatabase3P` select original archive XML documents. The
document indexes reference controller scopes, fragment alternatives and ADB subdatabase
declarations in their original order. The export preserves the declared motion inputs.
Physical clip binding, runtime variant choice and additive composition are outside this
document's current projection.

Source-backed values expose `containsEncryptedSource`, `sourceSetComplete` and
`sourceUsesRefs`. A join preserves both the original selector and observed target source.
Raw XML values use their document's physical identity and their original local position.
