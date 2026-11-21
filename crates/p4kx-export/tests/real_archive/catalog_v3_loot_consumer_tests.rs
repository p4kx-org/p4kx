// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_extraction::DataForge;
use p4kx_testkit::{TestRecord, TestStruct};
use serde_json::{Value, json};

use crate::catalog_v3_consumer as reader;
use crate::commodity_v3_fixture::{self as fixture, TestResult};

const KIND: Kind = Kind::LootPoolFilters;
const TYPE: &str = "PoolFilterRecord";
const GUID: u128 = 0xda01;
const TABLE: &str = "/definitions/lootPoolFilters";

#[test]
fn canonical_empty_loot_table_is_required_and_ignores_a_legacy_distraction() -> TestResult {
    let (_, mut catalog) = fixture::build(&fixture::database())?;
    assert!(
        reader::rows(&catalog, KIND)?.is_empty(),
        "no original Loot Records"
    );
    catalog.as_object_mut().ok_or("catalog object")?.insert(
        "lootPoolFilters".to_owned(),
        json!([{"guid": "legacy-distraction"}]),
    );
    assert!(
        reader::rows(&catalog, KIND)?.is_empty(),
        "the canonical empty table remains the only row source"
    );
    catalog
        .get_mut("definitions")
        .and_then(Value::as_object_mut)
        .ok_or("definitions")?
        .remove("lootPoolFilters")
        .ok_or("required actual Loot table")?;
    assert!(
        reader::rows(&catalog, KIND).is_err(),
        "a missing canonical table cannot become an empty success or use the legacy root"
    );
    Ok(())
}

#[test]
fn actual_loot_definitions_keep_sourced_guid_and_physical_header_checks() -> TestResult {
    let database = fixture::database()
        .structure(TestStruct::new(TYPE))
        .record(TestRecord::new(
            GUID,
            TYPE,
            "PoolFilterRecord.CanonicalConsumer",
            "libs/foundry/records/lootgeneration/filters/canonical_consumer.xml",
        ));
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let expected = fixture::guid(GUID);
    let record = forge
        .records()
        .find(|record| record.guid().to_string() == expected)
        .ok_or("actual original Loot Record")?;
    assert_eq!(record.type_name()?, TYPE, "the selected original family");
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the tree belongs to that actual Record"
    );
    assert!(
        tree.root().fields().is_empty(),
        "the original Record has no fields"
    );
    let (_, catalog) = fixture::build(&database)?;
    let rows = reader::rows(&catalog, KIND)?;
    assert_eq!(rows.len(), 1, "one actual physical Loot definition");
    let row = rows.first().ok_or("actual exported Loot row")?;
    assert_eq!(
        reader::identity(row, KIND)?,
        expected,
        "sourced original GUID"
    );
    assert_eq!(
        reader::field(row, KIND, "recordIndex")?,
        &json!(record.index()),
        "the actual physical original index is retained"
    );
    crate::real_archive_verification::check_unique_guids(&catalog)?;
    let mut missing_origin = catalog.clone();
    missing_origin
        .pointer_mut(&format!("{TABLE}/rows/0/guid"))
        .and_then(Value::as_object_mut)
        .ok_or("actual GUID cell")?
        .remove("sourceUsesRefs")
        .ok_or("required original GUID references")?;
    assert!(
        reader::rows(&missing_origin, KIND).is_err(),
        "an immediate source cell cannot lose its origin"
    );
    let mut wrong_header = catalog;
    *wrong_header
        .pointer_mut(&format!("{TABLE}/rows/0/recordIndex/value"))
        .ok_or("physical source index")? = json!("0");
    assert!(
        reader::rows(&wrong_header, KIND).is_err(),
        "the original physical index must retain its numeric payload type"
    );
    Ok(())
}
