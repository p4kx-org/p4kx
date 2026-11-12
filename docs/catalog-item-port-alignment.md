# Item-port alignment and attachment constraints

Catalog schema v3 keeps the port declaration and the attached item's declaration
in separate physical configuration rows. Both retain their original values and
per-value source provenance.

| Catalog path | Original DataForge location |
|---|---|
| `configurations.itemPorts.rows[].ports` | `EntityClassDefinition.Components[]` → `SItemPortContainerComponentParams.Ports[]` |
| `ports.value[].value.attachAxesToAlign` | `SItemPortDef.attachAxesToAlign` |
| `configurations.itemAttachmentConstraints.rows[].definition` | `EntityClassDefinition.Components[]` → `SAttachableComponentParams.AttachDef` |
| `definition.value.attachAxesToAlign` | `SItemDefinition.attachAxesToAlign` |
| `definition.value.ignoredAttachAxis` | `SItemDefinition.ignoredAttachAxis` |
| `definition.value.attachCheckCenterOfMass` | `SItemDefinition.attachCheckCenterOfMass` |

A configuration row identifies its physical Entity Record and original
`Components` ordinal. Port slots preserve their stored order and use
`portOrdinal` within that component. Names, including duplicate names, belong
to those individual slots. The array and scalar definition carry their own
presence states. The center-of-mass check retains a stored `false` as a
present value. A scalar StrongPointer that supplies a concrete
`SItemDefinition` also contributes its original fields.

Each axis field contains `label` and `optionStatus`. The label is the exact
string stored by the scalar `EItemPortAttachRotationLimitAxis` field. The status
compares the string with the options of the enum declaration in the same
DataForge input.

- `declared` identifies a matching original option. Its provenance points to
  that option and the original label.
- `undeclared` preserves a label outside the readable original option list.
- An unreadable option list produces an `unreadable` status when a readable
  option has not established a match.
- `optionStatus` is `null` when no readable label is available. The label's
  state identifies absence, a stored null, an unreadable value or unsupported
  storage.

Each nested Loadout port carries `sourceConstraints`. Its `portDefinition`
joins the DataForge definition the shared resolver selected. Its
`attachedItem.targetGuid` and `attachedItem.targetRecordIndex` retain the original
Reference or class selection, including the XML attribute and file selectors
when XML supplies that choice. The physical Record index also joins the
attached Entity's fuel and resource configurations.
The attached item's `definition` joins the first attachable component that
supplies the item's definition. A `projected` link uses the canonical physical
row coordinates. `notProjected` means the source row is outside the available
configuration output, for example under a Record filter. `notDeclared` means
the resolved item has no attachable component. Empty and unresolved
attachments have a null `attachedItem`. XML-only ports have a null
`portDefinition`.

The resolver continues to place attachments using its established helper
transforms and merge rules. The alignment labels and center-of-mass declaration
are available for inspection with the other source configuration.
