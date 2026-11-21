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
An empty array retains its original declared target. An unrelated Class target is unsupported
even when it has no elements. The table assigns no precedence to repeated names. Absence, explicit null, unreadable values,
unsupported concrete types and invalid storage remain distinct source states.

An Entity's original Tag references are in declarations.entityTags. Join the sourced
ownerRecordIndex to that declaration's recordIndex to find its ordered Tag references,
including dangling targets. This keeps Entity root tags distinct from attachable tags and
from modifier names.

An authored chip modifier named `errorChance` remains an ordered value.
The target Entity owns `HackableParams.baseErrorChance`.

## Hackable devices

`configurations.hackableDevices` retains every selected `HackableParams` occurrence by
physical Record and Components slot, including Entities outside the Item selection.
`baseDuration`, `baseErrorChance`, `maxPauseDuration` and `updateProgressDeltaTime`
retain their exact `Single` bits and finite value. The declarations establish the
storage type. Units, scaling and the runtime combination with chip modifiers are
unconfirmed. `numErrorChecks` is a stored unsigned count and `resetAfterHackSuccess`
is the original flag. Zero values remain present.

The four `audioRtpc*` fields and nine `audioTrigger*` fields retain their original
class, `rtpc` or `audioTrigger` identifier, and any additional members. Empty identifiers
remain present. Every typed value carries its own origin and source state.

`configurations.interactions` retains the `SHackableStateModifier` variant as
`kind: "hacking"` in the original state category and array position. Its
`onHackingErrorState`, `onHackingResetState` and `onHackingSuccessState` are weak state
links. Join by `ownerRecordIndex` to inspect the state machine and hackable settings
on the same Entity. This join identifies the owning Entity and cannot select an
effective modifier or determine a hack result.

## Real Archive verification

With `P4KX_DATA_P4K` configured, the opt-in test
`hacking_chip_components_match_actual_item_slots_and_modifier_sources` in the
`p4kx-export` `real_archive` test binary checks the original Item slots, authored
values, declarations and DCB provenance. Set `P4KX_EXPECT_HACKING_COMPONENTS` to
require both chip families and at least one original modifier for a positive
acceptance run. `P4KX_HACKING_COMPONENT_FILTER` can select a known Record path or
an Item subtree. The test prints counts only.

For a target-device check, set `P4KX_HACKABLE_DEVICE_RECORD` to the exact Record path
of a hackable Entity with a resolved hacking outcome state. With the same Archive
configuration, run the following test. It compares the complete known `HackableParams`
fields, the original state targets and their source coordinates.

```powershell
cargo test -p p4kx-export --test real_archive --locked hackable_device_keeps_original_target_values_and_state_links -- --ignored
```
