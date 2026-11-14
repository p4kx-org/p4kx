# Consumables in the P4KX catalog

`configurations.consumables` contains each original `SCItemConsumableParams`
component. A row identifies its physical Entity Record and Components ordinal. A Record
can contain several components. Selection uses the owner's normal Record filter.

`defaultContents` preserves each class-array slot in its stored order. Each slot
retains its original `consumableSubtype` Reference and exact Single `ratio`.
`consumableVolume` keeps the concrete authored cargo unit. For example,
`microSCU` in `SMicroCargoUnit` is an Int32. Container, lid and utensil tags
retain empty strings. Closure, reclosure, discard, one-shot, prone, animation and
two-handed flags retain authored false. `additionalFields` includes original
attachment geometry and materials, animation fragment, switch, helper and interaction
declarations in their stored types and order.

`definitions.consumableTypes` and `definitions.consumableSubtypes` use the common
physical Record header. A type owns an ordered array of subtype References. A filtered
Catalog includes the subtype targets of selected types and selected consumable components
once by physical Record index, along with directly selected subtypes. A dangling link
retains its authored GUID. `typeName` is the original classification token.
`consumableName` identifies a public short localized name. The subtype retains its
tint override with the original concrete type and member values.

`effectsPerMicroSCU` retains each original StrongPointer position, including nulls.
Each effect owns an independent protected `effectDescription`, with the original
type and source declarations.

| Source type | Typed parameters |
| --- | --- |
| `ConsumableEffectModifyActorStatus` | `statType`, `statPointChange`, `statCooldownChange` |
| `ConsumableEffectHealth` | `healthChange` |
| `ConsumableEffectAddBuffEffect` | `buffType`, original duration/value override pointer and body, including indefinite variants |
| `ConsumableEffectResource` | `consumableResourceType`, `amount` |
| `ConsumableEffectGas` | `gasMass` with original gas Reference and mass |
| Additional concrete effect classes | `authored` with the original fields in the source arena |

Every stored Single retains `rawBits` and nullable `finite`. The typed fields, source
arena and original relationships retain immediate per-value origins. A source enum does
not imply a physical Buff or resource Record when the original value has no Reference.
The source rate remains per micro-SCU as named by the declaration. Consumption totals,
unit conversions and gameplay outcomes are left to consumers.

## Verification

The opt-in `real_archive_consumables` tests read the build's original DataForge and the
actual Catalog writer. Set `P4KX_CONSUMABLE_SUBTYPE_FILTER` to a subtype Record path and
`P4KX_CONSUMABLE_OWNER_RECORD` to an exact Entity Record path with populated composition.
The tests check physical scope, ordered effect and composition slots, authored flags and
volume, and exact numeric storage. The effect comparison also checks direct source-use
coordinates against DataForge. The synthetic cases exercise empty lists, null slots,
dangling links, a cyclic type/subtype relationship, and an additional authored effect class.
