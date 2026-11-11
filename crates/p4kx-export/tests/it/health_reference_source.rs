// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, Field, Record, RecordTree, Value};

use crate::health_origin_fixture::{self as fixture, TestResult};
use crate::health_reference_fixture as source;

pub(crate) fn original(bytes: &[u8]) -> TestResult {
    let forge = DataForge::open(bytes)?;
    let target = forge.record(0).ok_or("actual original target")?;
    let owner = forge.record(1).ok_or("actual original owner")?;
    assert_eq!(
        owner.guid().to_string(),
        fixture::guid(source::OWNER),
        "original owning GUID"
    );
    assert_eq!(
        target.guid().to_string(),
        fixture::guid(source::TARGET),
        "different target GUID"
    );
    assert_eq!(
        owner.record_path()?,
        source::OWNER_PATH,
        "actual normalized owner path"
    );
    assert_eq!(
        target.record_path()?,
        source::target_path(),
        "complete actual long target path"
    );
    assert_eq!(
        target.type_name()?,
        source::TYPE,
        "actual catalog target type"
    );
    let tree = owner.tree()?;
    assert!(
        tree.is_from(&owner),
        "the original tree carries actual owner authority"
    );
    let fields: Vec<_> = tree.root().fields().iter().map(Field::name).collect();
    assert_eq!(
        fields,
        ["Code", "Left", "Right"],
        "original declaration and DFS section order"
    );
    arrays(&owner, &target, &tree)?;
    assert!(
        bytes.len() < 64 * 1024,
        "the source uses a modest single long spelling, not an OOM fixture"
    );
    Ok(())
}

fn arrays(owner: &Record<'_>, target: &Record<'_>, tree: &RecordTree<'_>) -> TestResult {
    for (name, count) in [("Left", source::LEFT), ("Right", source::RIGHT)] {
        let field = tree
            .root()
            .fields()
            .iter()
            .find(|field| field.name() == name)
            .ok_or("actual original field declaration")?;
        assert_eq!(
            field.storage().data_type(),
            DataType::Reference,
            "genuine authored GUID storage"
        );
        assert!(field.storage().is_array(), "genuine original array storage");
        let Some(Value::Array(values)) = tree.field(name) else {
            return Err("actual original Reference array".into());
        };
        assert_eq!(
            values.len(),
            count,
            "every original array slot exists before the mapper"
        );
        for value in values {
            reference(owner, target, value)?;
        }
    }
    Ok(())
}

fn reference(owner: &Record<'_>, target: &Record<'_>, value: &Value<'_>) -> TestResult {
    let Value::Reference(reference) = value else {
        return Err("actual original resolved Reference slot".into());
    };
    assert_eq!(reference.record_index(), 0, "actual physical target");
    assert_eq!(
        reference.guid(),
        target.guid(),
        "actual stored GUID resolves to that target"
    );
    assert_eq!(
        reference.record_path(),
        target.record_path()?,
        "original full target spelling"
    );
    assert!(
        reference.location().is_none(),
        "a genuine Record Reference, not an instance pointer"
    );
    assert!(
        reference.original_target().shares_source_with(owner),
        "original target admission shares this Forge"
    );
    Ok(())
}
