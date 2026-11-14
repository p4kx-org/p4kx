# Quantum interdiction generators

The Catalog v3 table at configurations.quantumInterdiction retains
one row per physical item component. Its owner Record index and component ordinal
identify the source item and slot. Device power fractions, jammer and pulse fields,
and weak interaction links keep their source states and per-value source-use
references. Zero, null and absent values are distinct.

visualGraph retains the authored state and transition array order. Its animation
fragment and tag are stored strings. Time-based transition parameters retain the
stored seconds, and value-based parameters retain the original context and
variable labels. Other concrete transition classes retain their source type and
original additional fields. References describe their original declaration and
target disposition. The export keeps the original parameters for consumers to
interpret without selecting a runtime transition.

Additional fields preserve original members outside the occurrences represented
by typed cells. An incompatible nominal declaration remains visible in diagnostic
partial exports.
