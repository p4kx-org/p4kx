// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from, is_kind_leftover,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, RecordTree, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::authored_definition_witness::{field, verify_array, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_auto_spawns::original::Locations;
use crate::real_archive_auto_spawns::provenance::Origins;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct Family {
    kind: Kind,
    source_type: &'static str,
    field: Option<OriginalArray>,
}

enum OriginalArray {
    Scopes,
    Members {
        name: &'static str,
        target: &'static str,
    },
}

const FAMILIES: [Family; 6] = [
    Family {
        kind: Kind::LocationClasses,
        source_type: "MissionLocationClass",
        field: Some(OriginalArray::Scopes),
    },
    Family {
        kind: Kind::LocationFunctions,
        source_type: "MissionLocationFunction",
        field: None,
    },
    Family {
        kind: Kind::LocationSizes,
        source_type: "MissionLocationSize",
        field: None,
    },
    Family {
        kind: Kind::LocationClassRegistries,
        source_type: "MissionLocationClasses",
        field: Some(OriginalArray::Members {
            name: "locationClasses",
            target: "MissionLocationClass",
        }),
    },
    Family {
        kind: Kind::LocationFunctionRegistries,
        source_type: "MissionLocationFunctions",
        field: Some(OriginalArray::Members {
            name: "locationFunctions",
            target: "MissionLocationFunction",
        }),
    },
    Family {
        kind: Kind::LocationSizeRegistries,
        source_type: "MissionLocationSizes",
        field: Some(OriginalArray::Members {
            name: "locationSizes",
            target: "MissionLocationSize",
        }),
    },
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn mission_location_vocabulary_matches_original_records_and_ordered_slots() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    for family in FAMILIES {
        let Some(tree) = representative(&forge, &watch, &family)? else {
            report_missing(family.source_type)?;
            continue;
        };
        let catalog = selected_catalog(&archive, &entry, tree.record_path())?;
        let origins = Origins::read(&catalog, entry.origin())?;
        verify_family(&forge, &catalog, &family, &tree, &origins)?;
        watch
            .check()
            .map_err(|_| "Archive changed during vocabulary verification")?;
    }
    verify_classification(&archive, &entry, &forge, &watch)
}

fn report_missing(category: &str) -> TestResult {
    if std::env::var_os("P4KX_EXPECT_LOCATION_VOCABULARY").is_some() {
        return Err(format!("configured Archive lacks selected {category} source").into());
    }
    crate::real_archive_verification::print_line(&format!("no source-selected {category}"));
    Ok(())
}

fn representative<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
    family: &Family,
) -> TestResult<Option<RecordTree<'source>>> {
    let mut builder = TreeBuilder::new();
    let mut best = None;
    let mut count = 0;
    for record in forge.records() {
        watch.poll().map_err(|_| "Archive changed")?;
        if record.type_name().ok() != Some(family.source_type)
            || record
                .record_path()
                .map_err(|_| "original vocabulary Record path")?
                .contains(['*', '?'])
        {
            continue;
        }
        let tree = builder
            .build(&record)
            .map_err(|_| "read original vocabulary body")?;
        let source = match family.field {
            Some(OriginalArray::Scopes) => tree.field("scopesVisibleTo"),
            Some(OriginalArray::Members { name, .. }) => tree.field(name),
            None => return Ok(Some(tree)),
        };
        let population = match source {
            Some(Original::Array(items)) => items.len(),
            _ => 0,
        };
        if best.is_none() || population > count {
            count = population;
            best = Some(tree);
        }
    }
    Ok(best)
}

fn selected_catalog(archive: &Archive, entry: &DataForgeEntry, path: &str) -> TestResult<Value> {
    let filter = Filter::new(path);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build source-selected location vocabulary")?;
    let catalog = serde_json::from_slice(built.json()).map_err(|_| "read catalog JSON")?;
    crate::real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    Ok(catalog)
}

fn verify_family(
    forge: &DataForge<'_>,
    catalog: &Value,
    family: &Family,
    tree: &RecordTree<'_>,
    origins: &Origins<'_>,
) -> TestResult {
    let index = u64::from(tree.record_index());
    let guid = tree.guid().to_string();
    let rows = consumer::rows(catalog, family.kind)?;
    let mut selected = rows
        .iter()
        .filter(|row| row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(index));
    let row = selected
        .next()
        .ok_or("selected original vocabulary definition")?;
    assert!(
        selected.next().is_none()
            && consumer::identity(row, family.kind)? == guid
            && row.pointer("/recordPath/value").and_then(Value::as_str) == Some(tree.record_path()),
        "exact physical definition and original Record identity"
    );
    let locations = Locations::read(tree.root());
    match family.field {
        Some(OriginalArray::Scopes) => array(
            tree.field("scopesVisibleTo"),
            field(row, "scopesVisibleTo")?,
            tree.record_index(),
            &locations,
            origins,
            verify_scope,
        ),
        Some(OriginalArray::Members { name, target }) => array(
            tree.field(name),
            field(row, "members")?,
            tree.record_index(),
            &locations,
            origins,
            |slot, row| verify_link(forge, target, slot, row),
        ),
        None => {
            if tree.root().fields().is_empty() {
                let authored = field(row, "authoredFields")?;
                assert!(
                    field(authored, "roots")?
                        .as_array()
                        .is_some_and(Vec::is_empty),
                    "zero-field dictionary still has a physical identity"
                );
            }
            Ok(())
        }
    }
}

fn array<'source>(
    source: Option<&Original<'source>>,
    row: &Value,
    owner: u32,
    locations: &Locations<'source>,
    origins: &Origins<'_>,
    verify: impl Fn(Option<&Original<'source>>, &Value) -> TestResult,
) -> TestResult {
    if let Some(source) = source {
        origins.occurrence(row, owner, locations.ordinal(source)?, "$value")?;
    }
    verify_array(source, row, |source, row| {
        verify(source, row)?;
        if let Some(source @ (Original::Enum(_) | Original::Reference(_) | Original::Null)) = source
        {
            origins.occurrence(row, owner, locations.ordinal(source)?, "$value")?;
        }
        Ok(())
    })
}

const CLASSIFICATION: [(&str, &str, Kind); 3] = [
    (
        "locationClass",
        "MissionLocationClass",
        Kind::LocationClasses,
    ),
    (
        "locationFunction",
        "MissionLocationFunction",
        Kind::LocationFunctions,
    ),
    ("locationSize", "MissionLocationSize", Kind::LocationSizes),
];

fn classification_owners(forge: &DataForge<'_>, watch: &ArchiveWatch) -> TestResult<BTreeSet<u32>> {
    let mut selected = [None; CLASSIFICATION.len()];
    let mut builder = TreeBuilder::new();
    for record in forge.records() {
        watch.poll().map_err(|_| "Archive changed")?;
        if record.type_name().ok() != Some("MissionLocationTemplate") {
            continue;
        }
        let path = record
            .record_path()
            .map_err(|_| "original location template path")?;
        let name = record
            .name()
            .map_err(|_| "original location template name")?;
        if !path
            .replace('\\', "/")
            .to_ascii_lowercase()
            .starts_with("libs/foundry/records/missiondata/pu_locations/")
            || path.contains(['*', '?'])
            || is_kind_leftover(Kind::MissionLocationTemplates, path, name)
        {
            continue;
        }
        let tree = builder
            .build(&record)
            .map_err(|_| "read original location template")?;
        let Some(Original::Struct(data)) = tree.field("locationData") else {
            continue;
        };
        for (slot, (name, _, _)) in selected.iter_mut().zip(CLASSIFICATION) {
            if slot.is_none() && matches!(data.field(name), Some(Original::Reference(_))) {
                *slot = Some(record.index());
            }
        }
        if selected.iter().all(Option::is_some) {
            break;
        }
    }
    for ((name, _, _), owner) in CLASSIFICATION.into_iter().zip(selected) {
        if owner.is_none() {
            crate::real_archive_verification::print_line(&format!(
                "no source-selected {name} Reference"
            ));
        }
    }
    Ok(selected.into_iter().flatten().collect())
}

fn verify_classification(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    watch: &ArchiveWatch,
) -> TestResult {
    for index in classification_owners(forge, watch)? {
        let record = forge.record(index).ok_or("original location template")?;
        let tree = TreeBuilder::new()
            .build(&record)
            .map_err(|_| "read original location template")?;
        let catalog = selected_catalog(archive, entry, tree.record_path())?;
        let origins = Origins::read(&catalog, entry.origin())?;
        let rows = consumer::rows(&catalog, Kind::MissionLocationTemplates)?;
        let guid = tree.guid().to_string();
        let mut selected = rows.iter().filter(|row| {
            row.get("guid").and_then(Value::as_str) == Some(guid.as_str())
                && row.get("recordPath").and_then(Value::as_str) == Some(tree.record_path())
        });
        let row = selected
            .next()
            .ok_or("selected location classification owner")?;
        assert!(
            selected.next().is_none(),
            "one physical location classification owner"
        );
        compare_classification(forge, &catalog, &tree, row, &origins)?;
        watch.check().map_err(|_| "Archive changed")?;
    }
    Ok(())
}

fn compare_classification(
    forge: &DataForge<'_>,
    catalog: &Value,
    tree: &RecordTree<'_>,
    row: &Value,
    origins: &Origins<'_>,
) -> TestResult {
    let Some(source @ Original::Struct(data)) = tree.field("locationData") else {
        return Err("original selected MissionLocationData Class".into());
    };
    let classification = field(row, "classification")?;
    let positions = Locations::read(tree.root());
    let owner = tree.record_index();
    origins.occurrence(classification, owner, positions.ordinal(source)?, "$type")?;
    let classification = consumer::present(classification)?;
    for (name, target_type, kind) in CLASSIFICATION {
        let source = data.field(name);
        let cell = field(classification, name)?;
        verify_link(forge, target_type, source, cell)?;
        let Some(source @ Original::Reference(target)) = source else {
            continue;
        };
        let ordinal = positions.ordinal(source)?;
        origins.occurrence(cell, owner, ordinal, "$value")?;
        let target_row = consumer::present(cell)?;
        let index = field(target_row, "targetRecordIndex")?;
        assert!(
            consumer::present(index)?.as_u64() == Some(u64::from(target.record_index())),
            "classification keeps the original physical target index"
        );
        origins.reference_field(
            index,
            owner,
            ordinal,
            target.record_index(),
            Some("recordIndex"),
        )?;
        if consumer::rows(catalog, kind)?
            .iter()
            .all(|row| row.pointer("/recordIndex/value") != index.get("value"))
        {
            assert!(
                catalog
                    .pointer("/exportHealth/unresolvedReferences")
                    .and_then(Value::as_array)
                    .is_some_and(|rows| rows.iter().any(|row| row
                        .get("status")
                        .and_then(Value::as_str)
                        == Some("excludedByFilter")
                        && row.pointer("/targetGuid/value") == target_row.pointer("/guid/value"))),
                "the excluded physical classification target is explicit in export health"
            );
        }
    }
    Ok(())
}

fn verify_scope(source: Option<&Original<'_>>, row: &Value) -> TestResult {
    let Some(Original::Enum(value)) = source else {
        return verify_state(source, row);
    };
    assert_eq!(
        consumer::present(row)?.as_str(),
        Some(*value),
        "exact original scope label"
    );
    Ok(())
}
