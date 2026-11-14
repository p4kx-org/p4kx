// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::ops::ControlFlow;
use std::sync::Arc;

use p4kx_dataforge::{DataForge, RecordTree, StructValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{LoadoutPath, LoadoutPathObserver, LoadoutPathOperation, LoadoutSelection, PathRead};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Default)]
struct Probe(RefCell<Vec<LoadoutSelection>>);

impl LoadoutPathObserver for Probe {
    fn record_path(
        &self,
        selection: &LoadoutSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()> {
        assert!(
            parent
                .fields()
                .iter()
                .any(|field| std::ptr::eq(field.value(), value)),
            "the callback receives the actual selected field before its copy"
        );
        self.0.borrow_mut().push(selection.clone());
        ControlFlow::Continue(())
    }

    fn record_name(
        &self,
        _selection: &LoadoutSelection,
        _tree: &RecordTree<'_>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
}

fn database(kind: TestType, value: &str) -> Vec<u8> {
    DataForgeBuilder::new()
        .enumeration("Paths", [value])
        .structure(TestStruct::new("Holder").field("path", TestType::Class("Path".to_owned())))
        .structure(TestStruct::new("Path").field("loadoutPath", kind))
        .record(
            TestRecord::new(1, "Holder", "holder", "synthetic/holder.xml").value(
                "path",
                TestValue::Struct(
                    TestInstance::new("Path").value("loadoutPath", TestValue::text(value)),
                ),
            ),
        )
        .build()
}

#[test]
fn whole_pairs_keep_selection_and_shared_text_alone_cannot_rebind_it() -> TestResult {
    let bytes = database(TestType::String, "synthetic/shared.xml");
    let forge = DataForge::open(&bytes)?;
    let tree = forge
        .record(0)
        .ok_or("the source Record is absent")?
        .tree()?;
    let Some(Value::Struct(parent)) = tree.field("path") else {
        return Err("the actual parent struct is absent".into());
    };
    let probe = Probe::default();
    let reader = PathRead::new(Some(&probe), None);
    let path = reader.field(parent, "loadoutPath", LoadoutPathOperation::Copy);
    let again = reader.field(parent, "loadoutPath", LoadoutPathOperation::Copy);
    let clone = path.clone();
    let text_only = LoadoutPath::unbound(Arc::clone(path.text()));
    assert_eq!(path, again, "path equality remains value based");
    assert_eq!(path, text_only, "unbound equal text keeps value equality");
    assert_eq!(
        path.selection(),
        clone.selection(),
        "a whole clone keeps its genuine selection"
    );
    assert_ne!(
        path.selection(),
        again.selection(),
        "separate getters have separate selections"
    );
    assert!(
        text_only.selection().is_none(),
        "a shared text Arc cannot mint an authored selection"
    );
    assert_eq!(
        probe.0.borrow().len(),
        2,
        "only actual observed getters mint selections"
    );
    drop(tree);
    assert_eq!(
        clone.value(),
        "synthetic/shared.xml",
        "the pair outlives the borrowed canonical tree"
    );
    Ok(())
}

#[test]
fn string_locale_and_enum_follow_the_actual_value_text_contract() -> TestResult {
    for kind in [
        TestType::String,
        TestType::Locale,
        TestType::Enum("Paths".to_owned()),
    ] {
        let bytes = database(kind, "  synthetic/shared.xml  ");
        let forge = DataForge::open(&bytes)?;
        let tree = forge
            .record(0)
            .ok_or("the source Record is absent")?
            .tree()?;
        let Some(Value::Struct(parent)) = tree.field("path") else {
            return Err("the actual parent struct is absent".into());
        };
        let probe = Probe::default();
        let copied = PathRead::new(Some(&probe), None).field(
            parent,
            "loadoutPath",
            LoadoutPathOperation::Copy,
        );
        let trimmed = PathRead::new(Some(&probe), None).field(
            parent,
            "loadoutPath",
            LoadoutPathOperation::Trim,
        );
        let unobserved =
            PathRead::new(None, None).field(parent, "loadoutPath", LoadoutPathOperation::Copy);
        assert_eq!(
            copied.value(),
            "  synthetic/shared.xml  ",
            "Copy preserves the accepted datum"
        );
        assert_eq!(
            trimmed.value(),
            "synthetic/shared.xml",
            "Trim preserves the vehicle getter's operation"
        );
        assert_eq!(
            trimmed.selection().map(LoadoutSelection::operation),
            Some(LoadoutPathOperation::Trim),
            "the transformation is part of the authentic selection"
        );
        assert!(
            unobserved.selection().is_none(),
            "the ordinary getter allocates no selection token"
        );
    }
    Ok(())
}
#[test]
fn vehicle_value_equality_does_not_depend_on_record_name_authorship() -> TestResult {
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            p4kx_testkit::TestEntity::new(1, "Vehicle", "synthetic/vehicle.xml")
                .vehicle(
                    "Scripts/Entities/Vehicles/Implementations/Xml/vehicle.xml",
                    "test",
                )
                .record(),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge
        .record(0)
        .ok_or("the actual vehicle Record is absent")?
        .tree()?;
    let probe = Probe::default();
    let plain = crate::spec::EntitySpec::read(&tree, &PathRead::new(None, None));
    let observed = crate::spec::EntitySpec::read(&tree, &PathRead::new(Some(&probe), None));
    let plain = plain.vehicle.ok_or("the unobserved vehicle is absent")?;
    let observed = observed.vehicle.ok_or("the observed vehicle is absent")?;
    assert!(
        plain.class_name.is_none(),
        "the ordinary read has no class selection token"
    );
    assert!(
        observed.class_name.is_some(),
        "the observed read retains its actual class-name selection"
    );
    assert_eq!(
        plain, observed,
        "vehicle semantics remain independent of the optional source observer"
    );
    Ok(())
}

#[path = "loadout_missing_path_tests.rs"]
mod missing_path;
