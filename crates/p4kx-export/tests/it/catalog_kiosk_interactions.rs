// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::interactions::with_interactions_from;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, SourceValueState};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestInstance, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const COMPONENT: &str = "KioskInteractionEntityComponentParams";
const FLAG: &str = "supportWalkToAlignInVR";
const EXTENSION: &str = "fixtureExtension";

#[test]
fn kiosk_flag_preserves_original_presence_and_has_catalog_sdk_parity() -> TestResult {
    for (flag, expected_state, expected_value, state) in [
        (
            Some((TestType::Bool, TestValue::Bool(false))),
            "present",
            Some(false),
            SourceValueState::Present,
        ),
        (None, "absent", None, SourceValueState::Absent),
        (
            Some((TestType::String, TestValue::text("unsupported flag syntax"))),
            "invalid",
            None,
            SourceValueState::Invalid,
        ),
    ] {
        let (_directory, archive) = common::open(&database(flag), DcbOrigin::Encrypted)?;
        let entry = DataForgeEntry::read(&archive)?;
        let mut options = common::options();
        if state == SourceValueState::Invalid {
            options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        }
        let catalog: Value =
            serde_json::from_slice(build_catalog_from(&archive, &entry, &options)?.json())?;
        let [row] = proof::rows(&catalog, "/configurations/interactions/rows")? else {
            return Err("expected one physical kiosk component".into());
        };
        assert_eq!(row["kind"], "kiosk");
        proof::present(row, "ownerRecordIndex", &json!(0), Some(true))?;
        proof::present(row, "componentOrdinal", &json!(1), Some(true))?;
        proof::present(row, "sourceType", &json!(COMPONENT), Some(true))?;
        let cell = proof::cell(row, FLAG, expected_state)?;
        assert_eq!(cell["value"], json!(expected_value));
        if let Some(value) = expected_value {
            proof::present(row, FLAG, &json!(value), Some(true))?;
        }
        let extension = proof::at(row, "/additionalFields/roots/0")?;
        proof::present(extension, "name", &json!(EXTENSION), Some(true))?;
        jsonschema::validator_for(&p4kx_export::catalog_schema())?
            .validate(&catalog)
            .map_err(|error| format!("kiosk catalog schema: {error}"))?;
        proof::closure(&catalog)?;

        check_sdk_row(&archive, &entry, row, state, expected_value)?;
    }
    Ok(())
}

fn check_sdk_row(
    archive: &Archive,
    entry: &DataForgeEntry,
    row: &Value,
    state: SourceValueState,
    expected_value: Option<bool>,
) -> TestResult {
    with_interactions_from(
        archive,
        entry,
        CatalogRecordSelection::RecordIndex(0),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| -> TestResult {
            let [component] = snapshot.rows() else {
                return Err("expected one kiosk on the selected physical owner".into());
            };
            assert_eq!(
                &serde_json::to_value(component)?,
                row,
                "the same owner, field states and source references reach the SDK"
            );
            let flag = component
                .support_walk_to_align_in_vr()
                .ok_or("kiosk flag")?;
            assert_eq!(
                (flag.state(), flag.value().copied()),
                (state, expected_value),
                "typed SDK flag retains its original state and value"
            );
            Ok(())
        },
    )?
}

pub(super) fn database(flag: Option<(TestType, TestValue)>) -> DataForgeBuilder {
    let mut declaration = TestStruct::new(COMPONENT)
        .parent("DataForgeComponentParams")
        .field(EXTENSION, TestType::UInt32);
    let mut kiosk = TestInstance::new(COMPONENT).value(EXTENSION, TestValue::UInt32(3));
    if let Some((kind, value)) = flag {
        declaration = declaration.field(FLAG, kind);
        kiosk = kiosk.value(FLAG, value);
    }
    p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8))
        .structure(declaration)
        .record(
            TestEntity::new(
                0x1048_0001,
                "FixtureKiosk",
                "libs/foundry/records/entities/scitem/fixture/kiosk.xml",
            )
            .attach_type("Misc")
            .component(TestValue::Struct(kiosk))
            .record(),
        )
}
