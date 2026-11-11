# Hacking chips

The configurations.hackingChips table retains one row per original chip component of a
selected Item. Its sourced ownerGuid, ownerRecordIndex and componentOrdinal identify the
physical Record and original Components slot. Rows are ordered by the physical Record and
slot. sourceType is the concrete declaration selected at the slot.

| Original declaration | Authored fields |
| --- | --- |
| HackingChipParams | maxCharges Int32, a count |
| RemovableChipParams | values Class array of RemovableChipValue |
| RemovableChipValue | name String and value Single |

The hackingChip configuration keeps the stored charge count, including zero. The removableChip
configuration keeps the original array and all its elements in order. Repeated names, empty
arrays and unrecognized names are valid source declarations. Names keep their exact original
text with the conservative protection for unclassified authored strings. Values keep their
exact Single bits and nullable finite decimal, with no common unit inferred from the name.
The table assigns no precedence to repeated names. Absence, explicit null, unreadable values,
unsupported concrete types and invalid storage remain distinct source states.

An Entity's original Tag references are in declarations.entityTags. Join the sourced
ownerRecordIndex to that declaration's recordIndex to find its ordered Tag references,
including dangling targets. This keeps Entity root tags distinct from attachable tags and
from modifier names.

The chip declaration has no direct errorChance field in the surveyed Build. An authored
modifier with that name remains an ordered value. HackableParams.baseErrorChance and
controller/state interactions have their own owning source.
