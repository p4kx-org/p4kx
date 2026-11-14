# Countermeasure emitters

`configurations.countermeasureEmitters` holds one route per physical weapon component,
container component, ammunition role and Ammo Record. Recognized launcher selections already
prove the route in `projections.weaponProjectileAmmoSelections`. Several launcher actions
selecting the same route contribute their origins to one emitter row. Primary and secondary
stay separate when both select the same Ammo Record. Rows follow physical weapon order, then
primary before secondary and the original target coordinates.

The row's `ownerRecordIndex` and `componentOrdinal` identify the weapon. Its sourced `route`
holds the following join coordinates.

| Field | Canonical target |
| --- | --- |
| `containerRecordIndex` and `containerComponentOrdinal` | `configurations.ammoContainers` by `ownerRecordIndex` and `componentOrdinal`. Initial count, capacity and restock retain their original authored values and widths. A linked target outside the filter may have physical coordinates without a selected row. |
| `ammoRole` | The same physical container and role in `declarations.ammoBindings`; `primary` selects `primaryAmmo` and `secondary` selects `secondaryAmmo`. |
| `ammoRecordIndex` | `definitions.ammoDefinitions` by `recordIndex`. The original countermeasure subtype and signature profile are under `projectileParams.value.kind.value.typeParams`. |

The route combines actual source evidence from the launcher selection, container relationship
and the countermeasure projectile. Classification follows the readable original
`CounterMeasureProjectileParams` structure. A null, invalid or future subtype may still be
visible as its own source state on the canonical Ammo definition. An unproved route, unreadable
projectile or Ammo definition outside the filter leaves the owned declarations and selections
available in their canonical tables. Availability reports whether the countermeasure source
type is declared. An available table may contain zero recognized rows.

Signature values retain their authored Single bits and source units. Chaff timing and radius
come from the canonical ammunition type parameters.

## Installed Archive verification

Set `P4KX_DATA_P4K`, `P4KX_COUNTERMEASURE_FILTER` and `P4KX_COUNTERMEASURE_AMMO`.
The filter selects a small Record subset containing the exact Ammo Record and an Item with a
same-owner weapon and container. The Ammo path comes from DataForge and omits the XML suffix.
Run the ignored test once per subtype to compare the physical route, all eight signature
fields and the three independent container counters with the original DataForge cells.
The test prints aggregate counts.

~~~powershell
cargo test -p p4kx-export --test real_archive countermeasure_emitter_matches_original_weapon_container_and_signature -- --ignored --nocapture
~~~
