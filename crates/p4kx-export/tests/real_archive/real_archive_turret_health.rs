// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::durability::with_turret_health_from;
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Filter, InputScope,
    RecordTree, SourceEncryption, SourceInput, SourceScope, SourceUses, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::component_sdk_parity::ComponentSdkParity;
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_verification::print_line;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const TURRET: &str = "SCItemTurretParams";
const MODIFIER: &str = "STurretHealthModifierDef";
const COMPONENT_ROWS: &str = "/configurations/turretHealth/rows";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn turret_health_matches_original_components_and_points() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut trees = TreeBuilder::new();
    let probe = forge
        .pointer_probe("Components", TURRET)
        .ok_or("turret component declaration")?;
    let owner = forge
        .records()
        .filter(|record| record.type_name() == Ok("EntityClassDefinition"))
        .find_map(|record| match trees.holds(&probe, &record) {
            Ok(false) => None,
            Ok(true) => Some(trees.build(&record)),
            Err(error) => Some(Err(error)),
        })
        .ok_or("original turret owner")??;
    let definition = forge
        .records()
        .find(|record| record.type_name() == Ok(MODIFIER))
        .ok_or("original turret health modifier definition")?
        .tree()?;
    let DataForgeOrigin::Archive(source) = entry.origin() else {
        return Err("DataForge Archive Entry".into());
    };
    let markers = SourceUses::with_compact_coordinates();
    let scope = markers.observe(SourceInput::Archive(source), InputScope::Payload)?;
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let owner_catalog = selected(&archive, &entry, &forge, &owner)?;
    let components = compare_components(&owner_catalog, &owner, &scope, encrypted)?;
    compare_sdk(&archive, &entry, &owner_catalog, owner.record_index())?;
    drop(owner_catalog);
    let definition_catalog = selected(&archive, &entry, &forge, &definition)?;
    let points = compare_curve(&definition_catalog, &definition, &scope, encrypted)?;
    watch.check().map_err(|_| "installed Archive changed")?;
    print_line(&format!(
        "turret health: {components} physical components and {points} ordered points checked"
    ));
    Ok(())
}

fn compare_sdk(
    archive: &Archive,
    entry: &DataForgeEntry,
    catalog: &Value,
    owner: u32,
) -> TestResult {
    let comparison = ComponentSdkParity::new(archive, entry, catalog)?;
    with_turret_health_from(
        archive,
        entry,
        CatalogRecordSelection::RecordIndex(owner),
        None,
        |snapshot| comparison.compare(COMPONENT_ROWS, owner, &snapshot),
    )
    .map_err(|_| "query turret health for the installed owner")??;
    Ok(())
}

fn selected(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selected: &RecordTree<'_>,
) -> TestResult<Value> {
    let path = selected.record_path();
    let filter = Filter::parse(&format!("*{path}"))?;
    assert_eq!(
        forge
            .records()
            .filter(|record| record.record_path().is_ok_and(|path| filter.matches(path)))
            .count(),
        1,
        "the scoped check selects one physical Record",
    );
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, options.with_full_provenance())
        .map_err(|_| "build catalog for one selected original Record")?;
    Ok(serde_json::from_slice(built.json())?)
}

fn compare_components(
    catalog: &Value,
    tree: &RecordTree<'_>,
    scope: &SourceScope,
    encrypted: Option<bool>,
) -> TestResult<usize> {
    let uses = proof::indexed(catalog, "/provenance/uses", "id")?;
    let trace = Trace::of(tree, &uses, scope, encrypted);
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original component array".into());
    };
    let rows = proof::rows(catalog, COMPONENT_ROWS)?;
    let expected = components
        .iter()
        .enumerate()
        .filter_map(|(index, raw)| match raw {
            Original::Struct(component) if component.type_name() == TURRET => {
                Some((index, raw, component))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!expected.is_empty(), "original turret component");
    assert_eq!(
        rows.len(),
        expected.len(),
        "every original selected turret component"
    );
    for (row, (index, raw, component)) in rows.iter().zip(expected) {
        assert_eq!(
            proof::number(row, "/ownerRecordIndex/value")?,
            u64::from(tree.record_index()),
            "physical turret owner",
        );
        assert_eq!(
            proof::number(row, "/componentOrdinal/value")?,
            u64::try_from(index)?,
            "original component position",
        );
        trace.check(&row["componentOrdinal"], raw, "conversion")?;
        let link = component
            .field("healthModifierRecord")
            .ok_or("original modifier Reference")?;
        let cell = &row["healthModifierRecord"];
        match link {
            Original::Null => assert_eq!(
                cell["state"], "null",
                "the authored null Reference survives"
            ),
            Original::Reference(target) => {
                assert_eq!(
                    cell["state"], "present",
                    "the authored Reference remains present"
                );
                assert_eq!(
                    proof::text(cell, "/value/guid/value")?,
                    target.guid().to_string(),
                    "the stored Reference GUID survives",
                );
            }
            _ => return Err("original health modifier Reference is not readable".into()),
        }
        trace.check(cell, link, "selector")?;
    }
    Ok(rows.len())
}

fn compare_curve(
    catalog: &Value,
    tree: &RecordTree<'_>,
    scope: &SourceScope,
    encrypted: Option<bool>,
) -> TestResult<usize> {
    let uses = proof::indexed(catalog, "/provenance/uses", "id")?;
    let trace = Trace::of(tree, &uses, scope, encrypted);
    let [definition] = proof::rows(catalog, "/definitions/turretHealthModifiers/rows")? else {
        return Err("one independently selected definition".into());
    };
    assert_eq!(
        proof::number(definition, "/source/recordIndex/value")?,
        u64::from(tree.record_index()),
        "physical original modifier Record",
    );
    let Some(curve @ Original::Struct(body)) = tree.field("damageMovementModifier") else {
        return Err("original BezierCurve".into());
    };
    let curve_cell = &definition["damageMovementModifier"];
    trace.part(curve_cell, curve, "$type", "selector")?;
    let row = &curve_cell["value"];
    let Some(raw_lut @ Original::Bool(use_lut)) = body.field("useLUT") else {
        return Err("original lookup table flag".into());
    };
    assert_eq!(
        row.pointer("/useLUT/value").and_then(Value::as_bool),
        Some(*use_lut),
        "the original lookup table flag survives"
    );
    trace.check(&row["useLUT"], raw_lut, "payload")?;
    let Some(raw_points @ Original::Array(points)) = body.field("points") else {
        return Err("original ordered points".into());
    };
    let slots = proof::rows(row, "/points/value")?;
    trace.check(&row["points"], raw_points, "selector")?;
    assert!(!points.is_empty(), "positive original point count");
    assert_eq!(points.len(), slots.len(), "original point count");
    for (point, slot) in points.iter().zip(slots) {
        let Original::Struct(point_body) = point else {
            return Err("original Vec2 point".into());
        };
        trace.part(slot, point, "$type", "selector")?;
        for axis in ["x", "y"] {
            let Some(original @ Original::Single(value)) = point_body.field(axis) else {
                return Err("original Vec2 Single axis".into());
            };
            let cell = slot
                .get("value")
                .and_then(|point| point.get(axis))
                .ok_or("exported Vec2 Single axis")?;
            assert_eq!(
                u32::from_str_radix(proof::text(cell, "/value/rawBits")?, 16)?,
                value.to_bits(),
                "the original {axis} coordinate retains its exact Single bits",
            );
            trace.check(cell, original, "payload")?;
        }
    }
    Ok(points.len())
}
