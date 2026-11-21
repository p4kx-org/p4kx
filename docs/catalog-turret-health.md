# Turret health in P4KX catalog v3

`configurations.turretHealth` exports every physical `SCItemTurretParams`
component. Use `(ownerRecordIndex.value, componentOrdinal.value)` to identify
the component within its build. `ownerGuid` retains the authored Entity GUID.

`healthModifierRecord` carries the component's scalar Reference declared for
`STurretHealthModifierDef`. When its status is `present`,
`healthModifierRecord.value.targetRecordIndex.value` identifies the matching
`source.recordIndex.value` in `definitions.turretHealthModifiers`. A null
Reference leaves the turret unlinked. A dangling Reference retains the
authored GUID and its target status. A declaration or resolved type mismatch
retains its own disposition and diagnostic.

The definitions table includes independently selected modifier Records and
the resolved physical Records required by selected components. Physical
Records with duplicate GUIDs stay distinct. The Reference follows the
DataForge resolution order.

Use `equipment::durability::with_turret_health` to query the turret components
by physical Entity Record index or GUID. A resolved modifier index can be
passed separately to `equipment::component_definitions::with_turret_health_modifiers`.
The component query reads the original Reference and target identity.
Definition bodies and curves are read by the definition query.

Each definition's `damageMovementModifier.value` contains the inline
`BezierCurve` with `useLUT` and the ordered `points.value` array of `Vec2`
cells. Each coordinate keeps the original Single's `rawBits` and nullable
`finite` value. This is the authored curve. The catalog supplies no evaluated
movement, aiming or damage value. `additionalFields` carries other authored
values at each level. Each sourced cell and source-arena value has its own
provenance and `containsEncryptedSource` marker.
