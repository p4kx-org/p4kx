# Weather configurations

`configurations.electricalBehaviors` is a schema v3 table of physical
`ElectricalBehavior` DataForge Records. A row keeps the original GUID, Record index and other
identity fields in `source`. The Record filter applies to this physical owner, and an
original source-family availability status is recorded separately from the number of rows.

The authored `lightning` Class contains the ordered `effects` Class array. Each effect retains
its ordered `targetModes` StrongPointer array. List cells keep physical position, null slots and
the exact concrete type. `LightningTargetMode_InternalPlanet` has the typed `internalPlanet`
payload kind. All modes carry the `enable` Bool inherited from `LightningTargetMode`.
The internal-planet payload preserves the concrete subtype's declared order of
`minimumInterval`, `referenceInterval`, `length`, `scaleMultiplier`, `intensity` and
`distanceRange`. The two Range fields retain their respective `minimum` and `maximum`
endpoints in authored order. The scalar settings and endpoints preserve their original
IEEE 754 Single bits and a decimal when finite. Other valid derived types have an `other`
payload and retain their type and original members. The data describes the authored rules
and leaves targeting, strike frequency and intensity calculation to the game.

Original members outside typed fields remain in each owner's `additionalFields` arena.
If a typed array has a malformed or unsupported slot, its original member also remains in
the arena. A sourced cell delivers its exact state, contributor uses and encryption marker
independently of other values.

The selective SDK route `catalog_queries::weather::with_electrical_behaviors` returns the same
canonical rows and original source inventory without producing a full catalog. Its
`RecordIndex` selection is scoped directly to one physical Record.
