# Particle library sources

`definitions.particleLibraries` is a catalog schema v3 table of physical XML Entries.
It selects files below `Data/Libs/Particles/` in the Archive using case-insensitive
paths and the XML extension, then requires a decoded `ParticleLibrary` root.
A catalog Filter applies to those Entry paths. Readable XML with a different root is
skipped. A malformed, lossy or unreadable candidate is reported in
`exportHealth.failedEntries` and affects publication in the same way as other
incomplete source reads. Diagnostic publication retains readable siblings.

`availability` is `available` when a library was observed, `absent` when every
candidate was checked and none was a library, and `unavailable` when no library was
observed and at least one candidate could not be checked, including by Filter or
incomplete directory. `completeness.particleLibraries` describes the Filter
separately. Rows follow physical Archive directory order. Each row has one
`identity` with the original Entry reference, index and paths.

`nodes` lists the XML elements depth first in sibling order, preserving repeated
elements. Its sourced `nodeIndex` is the original parser position in the Entry.
`parentNodeIndex` uses the same domain and is absent at the root. `name` retains the
decoded element name. `attributes` retains all decoded name/value occurrences in
source order. `text` retains direct decoded text occurrences in order. XML origin
uses address names and parent relationships by node and attribute data by node
and attribute ordinal. Text uses carry the node and occurrence position.

`rainOcclusion` is a sourced Boolean for `Params` immediately below
`Particles`, including nested declarations. It is `null` on other elements.
Within that scope the states are `present`, `absent` and `invalid`.
The lexical values `1` and `true` map to true, and `0` and `false` map to
false, ignoring surrounding whitespace. An invalid value retains its original
decoded token in `attributes`. Light tokens such as `SharedLight`, `StartLight`
and `TargetLight` are stored as authored attributes with their direct field origin.

These declarations describe particle-library authoring data. Runtime assignment,
probabilities, propagation, and audio links require separate source declarations.
Existing authored particle resource references remain paths.
