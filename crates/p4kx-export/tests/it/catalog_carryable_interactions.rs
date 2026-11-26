// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{CatalogRecordSelection, ResourceFloat32};
use p4kx_export::{CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, SourceValueState, SourcedValue};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestInstance, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::{health_origin_assertions as proof, sdk_full_proof as full_proof};

const COMPONENT: &str = "SEntityComponentCarryableParams";
const CONTAINER: &str = "SGripContainer";
const GRIP: &str = "SCarryableGrip";
const HAND: &str = "SGrip";
const PARAMETERS: &str = "SCarryParams";

#[derive(Clone, Copy)]
enum Grips {
    Present,
    Empty,
    Missing,
    WrongTarget,
}

#[test]
fn carryable_has_source_bound_order_dimensions_and_sdk_parity() -> TestResult {
    let (_directory, archive) = common::open(&database(Grips::Present), DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, common::options().with_full_provenance())?.json(),
    )?;
    let row = carryable(&catalog)?;
    let [first, second] = proof::rows(row, "/gripData/value/grips/value")? else {
        return Err("expected two ordered grip slots".into());
    };
    let first = proof::at(first, "/value")?;
    let second = proof::at(second, "/value")?;
    proof::present(first, "carryStyleID", &json!("FixtureFirst"), Some(true))?;
    proof::present(second, "carryStyleID", &json!("FixtureSecond"), Some(true))?;
    let hand = proof::at(first, "/handGrip/value")?;
    proof::present(hand, "optionalHelper", &json!("FixtureRight"), Some(true))?;
    let off_hand = proof::at(first, "/offHandGrip/value")?;
    proof::present(
        off_hand,
        "optionalHelper",
        &json!("FixtureLeft"),
        Some(true),
    )?;
    proof::cell(second, "offHandGrip", "null")?;

    let dimensions = proof::at(first, "/carryParameters/value")?;
    proof::present(
        dimensions,
        "width",
        &json!({"rawBits": "3e800000", "finite": 0.25}),
        Some(true),
    )?;
    proof::present(
        dimensions,
        "height",
        &json!({"rawBits": "80000000", "finite": -0.0}),
        Some(true),
    )?;
    proof::present(
        dimensions,
        "depth",
        &json!({"rawBits": "3f400000", "finite": 0.75}),
        Some(true),
    )?;
    check_original_extensions(row, first, hand)?;
    jsonschema::validator_for(&p4kx_export::catalog_schema())?
        .validate(&catalog)
        .map_err(|error| format!("carryable catalog schema: {error}"))?;
    proof::closure(&catalog)?;
    check_typed_sdk(&archive, &entry, row)
}

#[test]
fn carryable_distinguishes_empty_absent_and_wrong_target_grip_arrays() -> TestResult {
    for (grips, state) in [
        (Grips::Empty, "present"),
        (Grips::Missing, "absent"),
        (Grips::WrongTarget, "unsupported"),
    ] {
        let (_directory, archive) = common::open(&database(grips), DcbOrigin::Encrypted)?;
        let entry = DataForgeEntry::read(&archive)?;
        let mut options = common::options();
        if matches!(grips, Grips::WrongTarget) {
            options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        }
        let built = build_catalog_from(&archive, &entry, options.with_full_provenance())?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        let container = proof::at(carryable(&catalog)?, "/gripData/value")?;
        let cell = proof::cell(container, "grips", state)?;
        if matches!(grips, Grips::Empty) {
            assert_eq!(
                cell["value"],
                json!([]),
                "the stored empty array is present"
            );
            assert_eq!(
                cell["containsEncryptedSource"], true,
                "the authored empty array retains its encryption marker"
            );
        }
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn carryable(catalog: &Value) -> TestResult<&Value> {
    let [row] = proof::rows(catalog, "/configurations/interactions/rows")? else {
        return Err("expected one carryable component on the selected owner".into());
    };
    assert_eq!(row["kind"], "carryable", "the concrete kind is serialized");
    proof::present(row, "ownerRecordIndex", &json!(0), Some(true))?;
    proof::present(row, "componentOrdinal", &json!(1), Some(true))?;
    proof::present(row, "sourceType", &json!(COMPONENT), Some(true))?;
    Ok(row)
}

fn check_original_extensions(row: &Value, grip: &Value, hand: &Value) -> TestResult {
    for (unit, field) in [
        (row, "fixtureComponentExtra"),
        (grip, "fixtureGripExtra"),
        (hand, "gripID"),
    ] {
        let [root] = proof::rows(unit, "/additionalFields/roots")? else {
            return Err("the original arena retains only the unprojected member".into());
        };
        proof::present(root, "name", &json!(field), Some(true))?;
    }
    Ok(())
}

fn check_typed_sdk(archive: &Archive, entry: &DataForgeEntry, row: &Value) -> TestResult {
    let session = full_proof::from_entry(archive, entry, full_proof::NO_LANGUAGES, None)?;
    session.with_interactions(
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            let [component] = snapshot.rows() else {
                return Err("expected one carryable on the selected physical owner".into());
            };
            assert_eq!(
                &serde_json::to_value(component)?,
                row,
                "the scoped SDK preserves the same source evidence and values"
            );
            let grip_data = component
                .grip_data()
                .and_then(SourcedValue::value)
                .ok_or("gripData")?;
            let [first, second] = grip_data.grips().value().ok_or("grips")?.as_slice() else {
                return Err("expected the two original SDK grip slots".into());
            };
            let first = first.value().ok_or("first grip")?;
            let second = second.value().ok_or("second grip")?;
            assert_eq!(
                first.carry_style_id().value().copied(),
                Some("FixtureFirst"),
                "the SDK exposes the authored identifier"
            );
            let dimensions = first.carry_parameters().value().ok_or("carry parameters")?;
            assert_eq!(
                dimensions.height().value().map(ResourceFloat32::raw_bits),
                Some((-0.0f32).to_bits()),
                "the SDK retains the original single-precision bit pattern"
            );
            assert_eq!(
                second.off_hand_grip().state(),
                SourceValueState::Null,
                "a null off-hand pointer has its own state"
            );
            Ok(())
        },
    )?
}

fn database(grips: Grips) -> DataForgeBuilder {
    let (container, value) = grip_container(grips);
    p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8))
        .structure(container)
        .structure(
            TestStruct::new(COMPONENT)
                .parent("DataForgeComponentParams")
                .field("gripData", class(CONTAINER))
                .field("fixtureComponentExtra", TestType::Bool),
        )
        .structure(
            TestStruct::new(GRIP)
                .field("carryStyleID", TestType::String)
                .field("handGrip", class(HAND))
                .field("offHandGrip", TestType::StrongPointer(HAND.to_owned()))
                .field("carryParameters", class(PARAMETERS))
                .field("fixtureGripExtra", TestType::UInt32),
        )
        .structure(
            TestStruct::new(HAND)
                .field("gripID", TestType::String)
                .field("optionalHelper", TestType::String),
        )
        .structure(
            TestStruct::new(PARAMETERS)
                .field("width", TestType::Single)
                .field("height", TestType::Single)
                .field("depth", TestType::Single),
        )
        .record(
            TestEntity::new(
                0x1176_0001,
                "FixtureCarryable",
                "libs/foundry/records/entities/scitem/fixture/carryable.xml",
            )
            .attach_type("Misc")
            .component(TestValue::Struct(
                TestInstance::new(COMPONENT)
                    .value("gripData", TestValue::Struct(value))
                    .value("fixtureComponentExtra", TestValue::Bool(true)),
            ))
            .record(),
        )
}

fn grip_container(grips: Grips) -> (TestStruct, TestInstance) {
    let mut container = TestStruct::new(CONTAINER);
    let mut value = TestInstance::new(CONTAINER);
    match grips {
        Grips::Present => {
            container = container.array("grips", class(GRIP));
            value = value.value(
                "grips",
                TestValue::Array(vec![
                    grip("FixtureFirst", true),
                    grip("FixtureSecond", false),
                ]),
            );
        }
        Grips::Empty => {
            container = container.array("grips", class(GRIP));
            value = value.value("grips", TestValue::Array(Vec::new()));
        }
        Grips::Missing => {
            container = container.field("gripOverrideUsingLOS", TestType::Bool);
        }
        Grips::WrongTarget => {
            container = container.array("grips", class(HAND));
            value = value.value("grips", TestValue::Array(Vec::new()));
        }
    }
    (container, value)
}

fn grip(style: &str, off_hand: bool) -> TestValue {
    let off_hand = if off_hand {
        hand("FixtureLeft")
    } else {
        TestValue::Null
    };
    TestValue::Struct(
        TestInstance::new(GRIP)
            .value("carryStyleID", TestValue::text(style))
            .value("handGrip", hand("FixtureRight"))
            .value("offHandGrip", off_hand)
            .value(
                "carryParameters",
                TestValue::Struct(
                    TestInstance::new(PARAMETERS)
                        .value("width", TestValue::Single(0.25))
                        .value("height", TestValue::Single(-0.0))
                        .value("depth", TestValue::Single(0.75)),
                ),
            )
            .value("fixtureGripExtra", TestValue::UInt32(7)),
    )
}

fn hand(helper: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new(HAND)
            .value("gripID", TestValue::text("FixtureGrip"))
            .value("optionalHelper", TestValue::text(helper)),
    )
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
