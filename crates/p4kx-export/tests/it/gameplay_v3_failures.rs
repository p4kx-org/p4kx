// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::commodity_v3_fixture::DcbOrigin;
use crate::gameplay_v3_original as original;
use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_source as source;
use crate::gameplay_v3_support::{self as support, TestResult};
use crate::gameplay_v3_values as values;
use p4kx_export::{CatalogPublicationMode, Filter};
use p4kx_extraction::{Value as Raw, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::json;

#[test]
fn gameplay_v3_preserves_filter_accounting_and_refuses_unsupported_or_unreadable_original_sources()
-> TestResult {
    filtered_scope()?;
    unsupported_field()?;
    future_operation()?;
    repeated_ownership()?;
    invalid_pointer()?;
    unreadable_enum()?;
    absent_fields()?;
    strict_read_failure()
}

fn filtered_scope() -> TestResult {
    let database = values::simple()
        .record(values::record(
            values::SECOND,
            "Hidden",
            values::owned(schema::BASE),
        ))
        .record(values::record(
            values::SECOND + 1,
            "Test_Leftover",
            values::owned(schema::BASE),
        ))
        .record(
            TestRecord::new(
                values::SECOND + 2,
                schema::PROPERTY,
                "CraftingGameplayPropertyDef.Outside",
                "synthetic/outside.xml",
            )
            .value("propertyName", TestValue::text("@resource_name")),
        );
    source::basic(&database)?;
    assert_excluded_owner(&database)?;
    let filter = Filter::new("*/Primary");
    let (_, catalog) = support::build(
        &database,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        Some(&filter),
    )?;
    assert_eq!(
        support::rows(&catalog)?.len(),
        1,
        "ordinary Filter selects one owning definition"
    );
    support::count(&catalog, 1, 0)?;
    assert_eq!(
        support::at(&catalog, "/completeness/gameplayProperties")?,
        &json!({"status":"partial","filter":"*/Primary"}),
        "selection scope stays partial"
    );
    assert_eq!(
        support::at(&catalog, "/exportHealth/status")?,
        &json!("complete"),
        "selected/dependency reads are healthy"
    );
    let (_, full) = support::normal(&database)?;
    support::count(&full, 2, 1)?;
    assert_eq!(
        support::rows(&full)?.len(),
        2,
        "no type-wide expansion or leftover materialization"
    );
    Ok(())
}

fn assert_excluded_owner(database: &DataForgeBuilder) -> TestResult {
    source::with_tree(database, 3, |tree| {
        assert_eq!(
            tree.type_name(),
            schema::PROPERTY,
            "the excluded source really has the same type"
        );
        assert!(
            !tree.record_path().starts_with(values::FOLDER),
            "the existing prefix excludes this owner"
        );
        Ok(())
    })
}

fn unsupported_field() -> TestResult {
    let database = schema::custom(
        schema::property().field("unexpected", TestType::Bool),
        TestType::Single,
    )
    .record(
        values::record(
            values::FIRST,
            "Primary",
            values::scale(TestValue::Single(1.25)),
        )
        .value("unexpected", TestValue::Bool(false)),
    );
    source::with_tree(&database, 0, |tree| {
        assert_eq!(
            tree.root().field("unexpected"),
            Some(&Raw::Bool(false)),
            "the extra original field is actually readable"
        );
        Ok(())
    })?;
    let (_, catalog) = support::diagnostic(&database)?;
    support::row(&catalog, 0)?;
    support::diagnosis(
        &catalog,
        "unsupportedValue",
        0,
        "unexpected",
        schema::PROPERTY,
    )?;
    support::refused(&database, None)
}

fn future_operation() -> TestResult {
    let name = "SyntheticFutureDisplayOperation";
    let database = schema::schema()
        .structure(
            TestStruct::new(name)
                .parent(schema::NON_REF)
                .field("future", TestType::UInt16),
        )
        .record(values::record(
            values::FIRST,
            "Primary",
            TestValue::Struct(TestInstance::new(name).value("future", TestValue::UInt16(7))),
        ));
    source::with_tree(&database, 0, |tree| {
        let display = source::structure(tree.root().field("displayTransformation"))?;
        assert_eq!(
            display.type_name(),
            name,
            "actual original unknown concrete source"
        );
        assert_eq!(
            display.field("future"),
            Some(&Raw::UInt16(7)),
            "readable future data is not an empty base"
        );
        Ok(())
    })?;
    let (_, catalog) = support::diagnostic(&database)?;
    let display = support::at(
        support::row(&catalog, 0)?,
        "/displayTransformation/value/nodes/0",
    )?;
    assert_eq!(
        support::at(display, "/sourceType/value")?,
        &json!(name),
        "future source identity remains data"
    );
    support::state(support::at(display, "/payload")?, "unsupported")?;
    support::diagnosis(
        &catalog,
        "unsupportedValue",
        0,
        "displayTransformation",
        name,
    )?;
    original::check(&catalog, &database, 0)?;
    crate::gameplay_v3_unknown_origin::check(&catalog, &database)?;
    support::refused(&database, None)
}

fn repeated_ownership() -> TestResult {
    let scale = TestInstance::new(schema::SCALE)
        .named("original-scale")
        .value("scale", TestValue::Single(1.25));
    let database = schema::schema().record(values::record(
        values::FIRST,
        "Primary",
        values::sequence(vec![
            TestValue::Struct(scale),
            TestValue::pointer("original-scale"),
        ]),
    ));
    source::with_tree(&database, 0, |tree| {
        let display = source::structure(tree.root().field("displayTransformation"))?;
        let slots = source::array(display.field("transformations"))?;
        assert!(
            matches!(slots.first(), Some(Raw::Struct(_))),
            "the first original pointer owns the instance"
        );
        assert!(
            matches!(
                slots.get(1),
                Some(Raw::Error(ValueError::OwnedTwice { .. }))
            ),
            "the repeated original pointer is a typed ownership error"
        );
        Ok(())
    })?;
    let (_, catalog) = support::diagnostic(&database)?;
    let slot = support::at(
        support::row(&catalog, 0)?,
        &format!("{}/transformations/value/1", support::DISPLAY),
    )?;
    support::state(slot, "unreadable")?;
    assert_eq!(
        support::at(slot, "/sourceSetComplete")?,
        &json!(false),
        "unreadable payload is not complete data"
    );
    assert_eq!(
        support::at(
            support::row(&catalog, 0)?,
            "/displayTransformation/value/nodes"
        )?
        .as_array()
        .map(Vec::len),
        Some(2),
        "an OwnedTwice source slot never becomes a successful duplicate or back-edge"
    );
    original::check(&catalog, &database, 0)?;
    support::refused(&database, None)
}

fn invalid_pointer() -> TestResult {
    let pointer = [u32::MAX.to_le_bytes(), 0_u32.to_le_bytes()].concat();
    let database = schema::schema().record(values::record(
        values::FIRST,
        "Primary",
        TestValue::Raw(pointer),
    ));
    source::with_tree(&database, 0, |tree| {
        assert!(
            matches!(tree.root().field("displayTransformation"),
            Some(Raw::Error(ValueError::InvalidPointer { struct_index, instance_index }))
                if (*struct_index, *instance_index) == (u32::MAX, 0)),
            "the actual non-null original pointer has an invalid target, not a null/default instance"
        );
        Ok(())
    })?;
    let (_, catalog) = support::diagnostic(&database)?;
    let carrier = support::at(support::row(&catalog, 0)?, "/displayTransformation")?;
    support::state(carrier, "unreadable")?;
    assert!(
        support::at(carrier, "/value")?.is_null(),
        "an invalid original pointer cannot invent an arena"
    );
    support::diagnosis(
        &catalog,
        "unreadableSource",
        0,
        "displayTransformation",
        schema::PROPERTY,
    )?;
    original::check(&catalog, &database, 0)?;
    support::refused(&database, None)
}

fn strict_read_failure() -> TestResult {
    let database = schema::custom(schema::property().declared_size(1), TestType::Single).record(
        values::record(
            values::FIRST,
            "Primary",
            values::scale(TestValue::Single(1.25)),
        ),
    );
    let bytes = database.build();
    let forge = p4kx_extraction::DataForge::open(&bytes)?;
    assert!(
        forge
            .record(0)
            .ok_or("original strict-failure owner")?
            .tree()
            .is_err(),
        "the original canonical source truly cannot be read"
    );
    let (_, catalog) = support::diagnostic(&database)?;
    assert!(
        support::rows(&catalog)?.is_empty(),
        "strict source failure emits no counterfeit D row"
    );
    let health = crate::quality_v3_support::kind_health(&catalog, "gameplayProperties")?;
    assert_eq!(
        support::at(health, "/sourceRecords/omittedAfterReadFailure")?,
        &json!(1),
        "one original omission"
    );
    assert_eq!(
        support::at(health, "/sourceRecords/omittedAfterProjectionFailure")?,
        &json!(0),
        "no duplicate projection omission"
    );
    support::refused(&database, None)
}

fn absent_fields() -> TestResult {
    let root = TestStruct::new(schema::PROPERTY).field("propertyName", TestType::Locale);
    let database = schema::custom(root, TestType::Single).record(values::record(
        values::FIRST,
        "Primary",
        TestValue::Null,
    ));
    source::with_tree(&database, 0, |tree| {
        for field in ["displayTransformation", "unitFormat", "nameOverrides"] {
            assert!(
                tree.root().field(field).is_none(),
                "this source truly did not declare {field}"
            );
        }
        Ok(())
    })?;
    let (_, catalog) = support::normal(&database)?;
    let row = support::row(&catalog, 0)?;
    for field in ["displayTransformation", "unitFormat", "nameOverrides"] {
        support::state(support::at(row, &format!("/{field}"))?, "absent")?;
    }
    original::check(&catalog, &database, 0)?;
    Ok(())
}

fn unreadable_enum() -> TestResult {
    let condition = values::item_type(
        vec![TestValue::Raw(u32::MAX.to_le_bytes().to_vec())],
        Vec::new(),
    );
    let database = schema::schema().record(
        values::record(values::FIRST, "Primary", values::owned(schema::BASE)).value(
            "nameOverrides",
            TestValue::Array(vec![values::name_override("@resource_name", condition)]),
        ),
    );
    source::with_tree(&database, 0, |tree| {
        let entry = source::structure(source::array(tree.root().field("nameOverrides"))?.first())?;
        let condition = source::structure(entry.field("condition"))?;
        assert!(
            matches!(
                source::array(condition.field("matchItemTypes"))?.first(),
                Some(Raw::Error(ValueError::InvalidString { .. }))
            ),
            "the original Enum label offset is actually unreadable"
        );
        Ok(())
    })?;
    let (_, catalog) = support::diagnostic(&database)?;
    let row = support::row(&catalog, 0)?;
    let slot = support::at(
        row,
        "/nameOverrides/value/0/value/payload/value/condition/value/payload/value/matchItemTypes/value/0",
    )?;
    support::state(slot, "unreadable")?;
    support::diagnosis(
        &catalog,
        "unreadableSource",
        0,
        "nameOverrides[0].condition.matchItemTypes[0]",
        schema::ITEM_TYPE,
    )?;
    support::refused(&database, None)
}
