// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Kind, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::catalog_v3_consumer as reader;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

const OWNER: u128 = 0x0404_1166;
const PATH: &str = "libs/foundry/records/missiondata/pu_locations/fixture/only";

#[test]
fn nonempty_location_root_rows_expose_sourced_identity_to_the_actual_acceptance_consumer()
-> TestResult {
    let (_folder, archive) = common::open(&database(), DcbOrigin::Plain)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, options.with_full_provenance())?;
    assert_eq!(
        built
            .summary()
            .kind(Kind::MissionLocationTemplates)
            .map(|kind| (kind.count, kind.skipped)),
        Some((1, 0)),
        "the owning producer admits the genuine location source"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    let rows = reader::rows(&catalog, Kind::MissionLocationTemplates)?;
    let [row] = rows.as_slice() else {
        return Err("one nonempty actual location root row".into());
    };
    assert_eq!(
        row.pointer("/recordIndex/value"),
        Some(&json!(0)),
        "the canonical row retains its exact original physical index"
    );
    assert_eq!(
        reader::identity(row, Kind::MissionLocationTemplates)?,
        common::guid(OWNER),
        "actual acceptance reads the sourced original GUID payload"
    );
    assert_eq!(
        reader::field(row, Kind::MissionLocationTemplates, "className")?,
        &json!("Only"),
        "the same matcher reads sourced class metadata"
    );
    assert_eq!(
        reader::field(row, Kind::MissionLocationTemplates, "recordPath")?,
        &json!(PATH),
        "the same matcher reads the original normalized path"
    );
    let mut unavailable = row.clone();
    unavailable["guid"]["state"] = json!("absent");
    unavailable["guid"]["value"] = Value::Null;
    assert!(
        reader::identity(&unavailable, Kind::MissionLocationTemplates).is_err(),
        "an unavailable sourced identity is never an empty successful GUID"
    );
    Ok(())
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("MissionLocationData"))
        .structure(TestStruct::new("MissionLocationTemplate").field(
            "locationData",
            TestType::Class("MissionLocationData".to_owned()),
        ))
        .record(
            TestRecord::new(
                OWNER,
                "MissionLocationTemplate",
                "MissionLocationTemplate.Only",
                format!("{PATH}.xml"),
            )
            .value(
                "locationData",
                TestValue::Struct(TestInstance::new("MissionLocationData")),
            ),
        )
}
