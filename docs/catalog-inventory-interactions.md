# Authored inventory interactions

`configurations.interactions` contains one row for each selected physical
`SEntityInteractableParams`, `SInteractionStateMachineParams` or
`KioskInteractionEntityComponentParams` or `SEntityComponentCarryableParams` component.
Its owner is identified by Record GUID,
physical Record index and component ordinal. Repeated declarations keep their original positions.

An `interactable` row retains its ordered shared declarations, interaction points, three task
families and menu title. Shared interaction identities, flags, display enums, tag links and the
two predicate lists retain their individual origin cells. An interaction point keeps its
WeakPointer to the authored shared object, alongside its helper, geometry and presentation fields.
Localized `DisplayName` values use the catalog game-text domain. Authored `Name` identifiers
remain source strings. Task selectors retain the authored strings and enum values. Concrete and inherited parameters remain in their original value arena.

A `stateMachine` row keeps state types, states, modifiers and transitions in source order. The
default, next-state, automatic transition, point and sequence links retain their actual WeakPointer
target. Record targets and owned-instance targets carry their original identities and locations.
The same target can appear in multiple independently sourced slots. Null pointers, missing members
and stored empty arrays have distinct states. Cargo occupancy modifiers retain their authored
vacant and occupied state links. Hacking modifiers retain the authored error, reset and success
state links in the same order and physical state category. [Hacking devices](catalog-hacking-chips.md#hackable-devices)

A `kiosk` row carries the original `supportWalkToAlignInVR` Bool from its component. The
sourced cell distinguishes a stored `false` from a missing field or invalid storage.
The SDK's `with_interactions` exposes the same row and the
`support_walk_to_align_in_vr()` accessor. Other authored kiosk members remain in
`additionalFields`. The flag describes the stored declaration.

On PTU 13260598, an independent read of one selected Item Record matched the
stored `false`, original Components position and payload provenance returned
by the SDK. Peak private commit was 1,115,238,400 bytes, and peak working set
was 1,034,334,208 bytes. Generic DataForge inspection covers Kiosk components
outside the Catalog's normal Entity selection.

A `carryable` row keeps the original `gripData` class and the ordered `grips` array. Each
grip preserves its `carryStyleID`, `handGrip` and optional `offHandGrip` helper, and
`carryParameters` width, height and depth as sourced Single values with original bits.
The SDK exposes the same row through `with_interactions` and `grip_data()`. Stored empty
grip arrays and absent or unsupported declarations have separate source states. The authored
parameters describe carrying, with no unit or pose inferred from their values.

Polymorphic predicates, dispatchers, animated parameters and other modifiers retain their
concrete source types and raw original members. Additional fields also preserve point
presentation and unmodeled source extensions. The inventory configuration's interaction link
can join to the same physical owned instance as a point or state link. These declarations
describe authoring data and carry the per-value encryption marker.
