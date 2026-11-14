// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from, is_kind_leftover,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, RecordTree, TreeBuilder, Value as Original,
    ValueError,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

use crate::catalog_v3_consumer as consumer;
use crate::real_archive_verification::print_line;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const FAMILIES: [(Kind, &str); 4] = [
    (Kind::StarmapObjects, "StarMapObject"),
    (Kind::StarmapTypes, "StarMapObjectType"),
    (Kind::StarmapPresets, "StarMapPreset"),
    (Kind::StarmapAmenities, "StarMapAmenityTypeEntry"),
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn original_starmap_types_presets_and_amenities_match_the_catalog() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build original starmap definitions")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    for (kind, source_type) in FAMILIES {
        let count = verify_family(&forge, &catalog, kind, source_type)?;
        print_line(&format!("{kind}: {count} original Records traced"));
    }
    watch
        .check()
        .map_err(|_| "Archive changed during starmap definition verification")?;
    Ok(())
}

fn verify_family(
    forge: &DataForge<'_>,
    catalog: &Value,
    kind: Kind,
    source_type: &str,
) -> TestResult<usize> {
    let expected: BTreeSet<_> = forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == source_type))
        .filter(|record| {
            kind != Kind::StarmapObjects
                || record
                    .record_path()
                    .ok()
                    .zip(record.name().ok())
                    .is_some_and(|(path, name)| {
                        kind.rule().is_some_and(|rule| rule.matches_path(path))
                            && !is_kind_leftover(kind, path, name)
                    })
        })
        .map(|record| record.index())
        .collect();
    let rows = consumer::rows(catalog, kind)?;
    let mut builder = TreeBuilder::new();
    let mut seen = BTreeSet::new();
    for row in rows {
        let index = u32::try_from(
            consumer::field(row, kind, "recordIndex")?
                .as_u64()
                .ok_or("physical starmap definition index")?,
        )?;
        assert!(seen.insert(index), "one row per physical definition");
        let record = forge.record(index).ok_or("original starmap Record")?;
        assert_eq!(record.type_name().ok(), Some(source_type), "original type");
        assert_eq!(
            consumer::identity(row, kind)?,
            record.guid().to_string(),
            "original GUID"
        );
        let tree = builder
            .build(&record)
            .map_err(|_| "read original starmap definition")?;
        verify_body(kind, &tree, row)?;
    }
    assert_eq!(seen, expected, "every original physical definition");
    Ok(rows.len())
}

fn verify_body(kind: Kind, tree: &RecordTree<'_>, row: &Value) -> TestResult {
    match kind {
        Kind::StarmapObjects => verify_object(tree, row),
        Kind::StarmapTypes => {
            for name in [
                "name",
                "spawnNavPoints",
                "showInMapSelectList",
                "validQuantumTravelDestination",
            ] {
                verify_scalar(tree.field(name), field(row, name)?)?;
            }
            Ok(())
        }
        Kind::StarmapPresets => {
            verify_reference(tree.field("type"), field(row, "type")?)?;
            for (name, fields) in [
                ("visibilitySettings", &["hideInWorld", "blockTravel"][..]),
                (
                    "displaySettings",
                    &["size", "minimumDisplaySize", "navIcon"][..],
                ),
                (
                    "quantumTravelData",
                    &["arrivalRadius", "obstructionRadius"][..],
                ),
                (
                    "locationParams",
                    &["excludeFromLevelLoad", "exposeForPlayerCreatedMissions"][..],
                ),
                ("radarSettings", &["isScannable"][..]),
            ] {
                let Some(Original::Struct(original)) = tree.field(name) else {
                    verify_state(tree.field(name), field(row, name)?)?;
                    continue;
                };
                let block = consumer::source_value(field(row, name)?)?
                    .ok_or("expected authored setting body")?;
                for member in fields {
                    verify_scalar(original.field(member), field(block, member)?)?;
                }
            }
            Ok(())
        }
        Kind::StarmapAmenities => {
            verify_scalar(tree.field("icon"), field(row, "icon")?)?;
            verify_reference(tree.field("requiredEntity"), field(row, "requiredEntity")?)
        }
        _ => Err("unhandled starmap definition family".into()),
    }
}

fn verify_object(tree: &RecordTree<'_>, row: &Value) -> TestResult {
    for name in [
        "hideInStarmap",
        "hideInWorld",
        "blockTravel",
        "isScannable",
        "noAutoBodyRecovery",
    ] {
        verify_scalar(tree.field(name), field(row, name)?)?;
    }
    for name in [
        "type",
        "parent",
        "jurisdiction",
        "affiliation",
        "locationHierarchyTag",
    ] {
        verify_reference(tree.field(name), field(row, name)?)?;
    }
    let amenities = field(row, "amenities")?;
    let Some(Original::Array(original)) = tree.field("amenities") else {
        return verify_state(tree.field("amenities"), amenities);
    };
    let rows = consumer::source_value(amenities)?
        .and_then(Value::as_array)
        .ok_or("ordered original amenity links")?;
    assert_eq!(rows.len(), original.len(), "every original amenity slot");
    for (row, slot) in rows.iter().zip(original.iter()) {
        verify_reference(Some(slot), row)?;
    }
    Ok(())
}

fn verify_scalar(source: Option<&Original<'_>>, row: &Value) -> TestResult {
    let expected = match source {
        Some(Original::Bool(value)) => json!(value),
        Some(Original::String(value) | Original::Enum(value)) => json!(value),
        Some(Original::Single(value)) => {
            return verify_float(row, &format!("{:08x}", value.to_bits()));
        }
        Some(Original::Double(value)) => {
            return verify_float(row, &format!("{:016x}", value.to_bits()));
        }
        _ => return verify_state(source, row),
    };
    assert_eq!(
        consumer::source_value(row)?,
        Some(&expected),
        "actual source scalar"
    );
    Ok(())
}

fn verify_float(row: &Value, bits: &str) -> TestResult {
    let payload = consumer::source_value(row)?.ok_or("original floating payload")?;
    assert_eq!(
        field(payload, "rawBits")?,
        &json!(bits),
        "original IEEE bits"
    );
    Ok(())
}

fn verify_reference(source: Option<&Original<'_>>, row: &Value) -> TestResult {
    let guid = match source {
        Some(Original::Reference(target)) => target.guid(),
        Some(Original::Error(ValueError::DanglingReference { guid })) => *guid,
        _ => return verify_state(source, row),
    };
    let link = consumer::source_value(row)?.ok_or("original reference payload")?;
    let actual = consumer::source_value(field(link, "guid")?)?;
    assert_eq!(
        actual,
        Some(&json!(guid.to_string())),
        "exact original GUID"
    );
    Ok(())
}

fn verify_state(source: Option<&Original<'_>>, row: &Value) -> TestResult {
    let expected = match source {
        None => "absent",
        Some(Original::Null) => "null",
        Some(Original::Error(_)) => "unreadable",
        Some(_) => "invalid",
    };
    assert!(
        consumer::source_value(row)?.is_none(),
        "original value is non-present"
    );
    assert_eq!(field(row, "state")?, expected, "original state");
    Ok(())
}

fn field<'row>(row: &'row Value, name: &str) -> TestResult<&'row Value> {
    row.get(name)
        .ok_or_else(|| format!("missing starmap field {name}").into())
}
