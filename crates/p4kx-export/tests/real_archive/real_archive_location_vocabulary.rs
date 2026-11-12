// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::authored_definition_witness::{field, verify_array, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_verification::print_line;

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
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build mission location vocabulary catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    for family in FAMILIES {
        let count = verify_family(&forge, &catalog, &family)?;
        print_line(&format!(
            "{}: {count} physical definitions checked",
            family.kind
        ));
    }
    watch
        .check()
        .map_err(|_| "Archive changed during vocabulary verification")?;
    Ok(())
}

fn verify_family(forge: &DataForge<'_>, catalog: &Value, family: &Family) -> TestResult<usize> {
    let mut expected: BTreeSet<_> = forge
        .records()
        .filter(|record| {
            record
                .type_name()
                .is_ok_and(|name| name == family.source_type)
        })
        .map(|record| (record.guid().to_string(), record.index()))
        .collect();
    let rows = consumer::rows(catalog, family.kind)?;
    let mut builder = TreeBuilder::new();
    for row in rows {
        let guid = consumer::identity(row, family.kind)?;
        let index = u32::try_from(
            consumer::field(row, family.kind, "recordIndex")?
                .as_u64()
                .ok_or("physical definition index")?,
        )?;
        let expected_identity = expected.pop_first().ok_or("extra physical row")?;
        assert_eq!(
            (guid, index),
            (expected_identity.0.as_str(), expected_identity.1),
            "physical identity and order"
        );
        let record = forge.record(index).ok_or("original vocabulary Record")?;
        let tree = builder
            .build(&record)
            .map_err(|_| "read original vocabulary body")?;
        match &family.field {
            Some(OriginalArray::Scopes) => {
                verify_array(
                    tree.field("scopesVisibleTo"),
                    field(row, "scopesVisibleTo")?,
                    verify_scope,
                )?;
            }
            Some(OriginalArray::Members { name, target }) => {
                verify_array(tree.field(name), field(row, "members")?, |slot, row| {
                    verify_link(forge, target, slot, row)
                })?;
            }
            None => {}
        }
    }
    assert!(expected.is_empty(), "all physical definitions were emitted");
    Ok(rows.len())
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
