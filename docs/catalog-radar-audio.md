# Radar audio configuration

`configurations.radarAudio` contains the physical root Archive Entries at
`Data/Libs/GameAudio/RadarAudioData.xml` with an unqualified `RadarAudioData` root.
The exact path accepts ASCII case variants and either separator. Repeated physical
Entries remain separate rows in directory order. The Catalog Filter applies to this
dataset. A selected Entry with an unreadable or invalid body is an export failure.

The ordered authored elements retain physical positions, exact qualified names,
ordered attributes and namespace declarations, and direct text. Typed fields cover
the default zoom range, category name and integral value, display time, starting
alpha, and sound-event slot name and value. Missing, explicit empty, and invalid
attributes have distinct states. The category value retains its exact integral
token including sign and leading zeros. Duplicate declarations of a recognised
field are invalid for the typed view and remain individually available in the
authored tree. Every value carries its own source and encryption evidence.

A sound-event element includes a sourced `bindingStatus` of `missingValue`,
`emptyValue`, `invalidValue`, or `unprovenSelector`. A nonempty slot supplies a
bare string. The authored hierarchy supplies no target selector or namespace for
that string. Candidate name comparisons alone cannot establish a runtime binding.
The status provenance covers the declaration of the slot and its value.
Querying this dataset reads the selected configuration XML Entries and directory
metadata without scanning other audio definitions, DataForge, media, or soundbanks.

`catalog_queries::radar_audio::read_radar_audio` accepts all canonical occurrences
or one physical root Entry index. It returns the canonical rows, observed physical
scope, source inventory, and failed read and projection counts.
