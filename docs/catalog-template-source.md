# Mission source and context

Catalog schema v3 uses definitions.contractTemplates as the physical owner table.
Each row has the common independently sourced Record header, plain inputRef navigation,
one source inventory, and typed missionFlows/missionProperties links. There is no root
contractTemplates dataset or second raw C inventory. These are the target contract and
implementation boundaries; generated-schema and runtime acceptance are separate checks.

## One original inventory

source.roots contains shared SourceField declarations together with required
sourcePosition{parentOrdinal,fieldIndex} controls. parentOrdinal is null only for an actual
Record-root field; a selected nested root retains its actual owning Struct ordinal even when that
parent is outside the emitted union. fieldIndex is its actual slot in that same Struct.fields list.
These original metadata are derived from the same sealed Fact as sourceOrdinal, not output indices. source.values is a flat sequence of
TemplateSourceValue objects ordered strictly by their actual full-Record sourceOrdinal.
A value has the common immediate SourcedValue<SourcePayload> cell, required-nullable
originalError, and required-nullable condition/action/randomPhase/trigger meanings.
SourceOrdinal and valueIndex are separate decimal controls.

The union retains missionFlow, contractProperties and objectiveTokens together with the actual
source sections selected by the existing Template projection: notForRelease, workInProgress,
contractDisplayInfo, contractClass, partialRewardPayout, modifiers, owner,
contractStartCommsNotification and contractEndCommsNotifications. All fields and every slot inside
these retained subtrees remain original facts, including readable wrong shapes and unknown types.
Only actually selected additional same-owner target-field subtrees are added. A retained ancestor
absorbs an overlapping selected root; otherwise roots are maximal retained actual fields.
Struct.fields and Array.elements are the sole original containment edges.

SourceFieldRef is root{rootFieldIndex} or field{parent:{valueIndex},fieldIndex}. A SourceValueRef is
{valueIndex}. Each decimal is a checked DecimalU64 control. PhysicalTemplateRef is
{inputRef,recordIndex}. These controls navigate already emitted data; they do not admit a source,
reconstruct a Scope, prove a physical owner or represent a game GUID.

A scalar's typed payload encodes that same datum under its one immediate value cell.
Independent declaration name/dataType/isArray/target, Struct sourceType, child value and original
error parameter facts have their own immediate sourced cells. Actual null keeps a null state and
an occurrence. Missing declaration instead has an Absent binding with real declaration-survey
evidence and no fabricated value node. All current original error variants and available parameters
remain per occurrence, independently of semantic Unreadable.

The shared dataforgeValue namespace uses actual Record index and full-Record DFS ordinal, including
declaration ordinals for declaration facts. Source coordinates do not use output valueIndex.
Each game datum exposes containsEncryptedSource, sourceUsesRefs and sourceSetComplete directly;
text protection remains independent. Exact widths, IEEE bits, false, zero, empty strings, GUID0,
null/error slots and repeated equal values remain intact. Unknown String/Locale domains use
the conservative protected Source text carrier; public short facts and approved UI formats keep
their established domains.

## Configuration and permanent context

configurations.missionFlows rows contain owner, declarationIndex, declaration, selection,
triggers and referencedFields. declaration is Present with a SourceFieldRef or a sourced Absent
binding. selection is absent, shadowedDeclaration, noStruct or selected{value}. referencedFields
is SourceFieldRef[], with no duplicated SourceField or raw values vector.

The shared MissionFlowOwnerRef is either
{type:"contractTemplate",inputRef,recordIndex,recordGuid} or
{type:"missionBrokerEntry",inputRef,recordIndex,recordGuid}. Both configurations.missionFlows and
each missionFlows declaration link use this same tagged physical navigation. Its components come
from the actual admitted owner; copying a link does not mint a receipt or authorize original values.

definitions.missionBrokerEntries rows retain one physical Broker header and every original root
and owned value in the same flat source arena as its Flow. The same actual occurrence supplies
both raw navigation and its separate required meanings. configurations.missionBrokerEntries rows
use a tagged physical broker link and the same objective, property, localization, Tag, modifier,
hauling and pool bindings as Templates. They also retain ordered reputation requirement operators
and the reward amounts selected from actual Reward Records. There is no root Broker DTO or
second raw arena.

Broker names, links, flags, settings, availability, schedule members, cooldowns, instance limits,
payout, buy-in and deadline quantities remain original cells in D. Month, repeat type, deadline
state, comparison and modifier meanings remain separate from their stored labels. A derived
reputation amount retains the actual selected target field and caller selection; its signed
64-bit decimal text preserves the integer exactly. A failed required Broker interpretation
keeps its D owner and declaration links while omitting its unsuccessful contextual row.

configurations.missionProperties rows contain owner, ownerObjective, declarationIndex,
declaration, selection and properties. owner uses the same tagged physical mission union as Flow.
ownerObjective is the actual objective occurrence's
SourceValueRef or null, never its authored id. selection also distinguishes shadowedObjective.
Existing Generator/MissionPropertySet identities, atomicity and registration remain separate.

configurations.contractTemplates rows permanently contain these established contextual outputs:

| Section | Representation |
| --- | --- |
| inputs | SourceField bindings for raw flags, display/class/settings, objectives, payout, modifiers, owner and comms |
| title/localizations | selected original source cell plus canonical GameText lookup and explicit empty/placeholder/not-key/null/wrong-shape/unreadable decisions |
| objectives/properties | physical source/preorder/parent links, original naming/value references and known property-kind controls |
| valueMeanings | recognized property-value operation, supported-shape result, typed condition/filter links and required interpretation failures |
| tagPaths | the actual captured Tag build's selected path or GUID fallback, caller contributions and explicit unavailable/unresolved/inapplicable decisions |
| propertyBindings/objectiveBindings/linkedOutputs | actual same-owner target-field references and existing interpretation controls, including the narrower MissionProperty target-type/status rule where applicable |
| vocabularies/modifiers | existing case-sensitive known labels and supported modifier decisions, linked to the unchanged original cells |
| owner | actual foreign Record-header class-name continuation from the same selected Reference receipt, including the original header failure |
| haulingOrders | established shadowing/grouping/level choices using actual selected original fields |
| spawnPools/poolBindings | established content IDs, empty-result decisions and first-variable/level selection linked to actual registrations and original variable/token fields |

Raw PartialPayout, ActiveSettings, AutoFinish, Handler, NavPoint and ordinary HaulingSettings
scalars are field references into D, not duplicate raw DTOs in C. Reward source has exactly its
actual currencyType, reward, max, plusBonuses and reputationBonus members; no inferred bounty field.

Contextual source work uses actual Record+Tree+OriginalInput callbacks and privately retained facts.
Weak targets use the decoder's real instance_positions plus actual typed Location steps/names.
Tag context borrows the same actual TagTree/NodeInputs pair; it does not build a second tree or
recover authority from Tag paths. The TemplateDependency spawn recipe uses the same bound content
adapter and exact existing ID payloads.

## Meanings, origin and failures

Condition/action/randomPhase/trigger meanings occupy the same owner nodes and point to their
existing raw cells. A successful selected target-field binding keeps exactly the actual target
field Payload, caller weak-slot Selector and genuine resolution Conversion, independently of the
raw cell's own contributions. Known nil/empty/null/wrong-shaped source facts remain their real
values with separate interpretation controls. Unresolved/error cases retain their actual original
inputs and completeness; no target/header GUID or successful target admission is invented.

`multipleActions` binds `actionsField` to the declared StrongPointer array and lists its
ordered owner-local `actions`. `pickRandomAction` binds `actionsField` to the declared
inline Class array and lists ordered `entries`. A declared entry includes the original `value`,
`actionField`, optional `action`, `probabilityField` and `probabilityInterpretation`. The
probability binding refers to the original Single cell with its exact IEEE-754 bits and origin.
The entry union distinguishes original null, unreadable, unsupported and invalid slots.
An authored null child has a raw value reference. An absent child declaration is expressed
through the action field's declaration survey. The same arena stores nested wrappers and
all readable child actions.

The opt-in source comparison `selected_flow_action_wrappers_match_the_original_owner_graph`
checks selected Template Records against their original action graphs. Set `P4KX_DATA_P4K`
for the installed Archive and `P4KX_EXPECT_FLOW_ACTION_WRAPPERS=1` when that Build must
contain both wrappers. Run the named test with `cargo test -p p4kx-export --test real_archive`
and `-- --ignored`.

Required unsupported meaning remains a failure. A fully emitted D owner stays successfully
projected while its required C interpretation reports DerivedProjection failure with the same
actual Record subject and original cause. Normal refuses; Diagnostic retains the readable raw
facts and Unsupported meaning and stays incomplete. Generator/MissionPropertySet atomicity is
unchanged.

Strict field/type guards capture their actual borrowed value/declaration and original parent
chain before constructing owned error text. Body recipes and real CatalogError causes survive
the Domain and SourceDiagnosis/DerivedFailure boundaries. A failed capture is not None, a
replacement FieldError or a reconstructed string-path receipt. Deferred unretained examples create
no Payload claim.

Only an actual emitted D owner occurrence can cover that same genuine number in a Template or
Broker, Contracts dependency or related pool/query input. Width, bits, original ordinal/path/type,
physical owner/input and cardinality remain checked. Matching GUIDs, values, local wire indices,
another occurrence or an un-emitted family cannot substitute for that receipt.

## Explicit consumer and oracle migration

The immutable WIRE09 storage form put values on missionFlows and referred to a separate selected-field
wrapper; the final owner form follows SCHEMA19 instead. Graph consumers first resolve the typed
physical D owner, then resolve SourceFieldRefs and its shared values. Per-configuration membership is
the original declaration plus its selected roots in that owner. Graph navigation changes, while
original source ordinals, typed values, errors, exact origins and physical overlap assertions stay
binding. sourcePosition additionally preserves the original parent/field metadata that a cut root's
local SourceFieldRef alone cannot reconstruct. Wrong genuine parent ordinals, wrong field indices,
foreign frames and overlapping aliases remain explicit negative controls. A selected field already inside a retained subtree is the same actual cell.

The original source-first depth/action/overlap fixtures remain evidence. Old root/recursive
behavior assertions must migrate explicitly to D.source/C links and the separate meaning controls.
The intentional Unknown change is SourceProjection to DerivedProjection with the same original
unsupported cause and retained raw values. No arbitrary alternative failure or missing assertion
satisfies that change.

Whole ordinary default-serde JSON and compressed JSON must parse the full Catalog envelope without
adding output recursion proportional to original owned source depth. No relaxed recursion limit, sampled success, substituted
depth quota, shadowed-field deletion or equal-value dedup establishes acceptance. API-dependent
Scope/receipt probes compile only after the real API exists and are not old semantic REDs.
