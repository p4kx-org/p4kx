# Authored inventory interactions

`configurations.interactions` contains one row for each selected physical
`SEntityInteractableParams` or `SInteractionStateMachineParams` component. Its owner is identified
by Record GUID, physical Record index and component ordinal. Repeated declarations keep their
original positions.

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
vacant and occupied state links.

Polymorphic predicates, dispatchers, animated parameters and other modifiers retain their
concrete source types and raw original members. Additional fields also preserve point
presentation and unmodeled source extensions. The inventory configuration's interaction link
can join to the same physical owned instance as a point or state link. These declarations
describe authoring data and carry the per-value encryption marker.
