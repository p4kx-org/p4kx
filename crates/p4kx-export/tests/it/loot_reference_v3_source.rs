// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForgeEntry, DataForgeOrigin, DataType, Location, Record, RecordTree, Reference,
    SourceEncryption, Value,
};

use crate::health_origin_fixture::{Input, TestResult};
use crate::loot_reference_v3_fixture as fixture;

pub(crate) fn check(entry: &DataForgeEntry, input: Input) -> TestResult {
    check_origin(entry, input);
    let forge = entry.open()?;
    assert_eq!(
        forge.records().len(),
        2,
        "two genuine physical Records before production"
    );
    let owner = forge.record(0).ok_or("actual owning Record")?;
    let target = forge.record(1).ok_or("actual selected target Record")?;
    assert_eq!(owner.index(), 0, "the actual source-owner Record position");
    assert_eq!(
        target.index(),
        1,
        "the actual source-target Record position"
    );
    let tree = owner.tree()?;
    let target_tree = target.tree()?;
    assert!(
        tree.is_from(&owner) && target_tree.is_from(&target),
        "actual canonical paired owners"
    );
    assert_eq!(
        owner.guid().to_string(),
        fixture::OWNER,
        "actual caller owner GUID"
    );
    assert_eq!(
        target.guid().to_string(),
        fixture::TARGET,
        "actual target GUID"
    );
    assert_eq!(
        target_tree.error_count(),
        0,
        "the independently copied target header has a readable owner"
    );
    check_declarations(&tree);
    check_owned(&tree)?;
    check_record_references(&owner, &tree)?;
    check_instance_references(&owner, &tree)
}

fn check_origin(entry: &DataForgeEntry, input: Input) {
    let expected = match input {
        Input::Plain => SourceEncryption::Unencrypted,
        Input::Encrypted => SourceEncryption::Encrypted,
        Input::Standalone => SourceEncryption::Unknown,
    };
    assert_eq!(
        entry.origin().encryption(),
        expected,
        "actual original input encryption"
    );
    assert_eq!(
        matches!(entry.origin(), DataForgeOrigin::File(_)),
        matches!(input, Input::Standalone),
        "unknown P4K lineage is a real standalone input"
    );
}

fn check_declarations(tree: &RecordTree<'_>) {
    let expected = [
        ("owned", DataType::StrongPointer),
        ("first", DataType::Reference),
        ("second", DataType::Reference),
        ("weakRecord", DataType::WeakPointer),
        ("weakRecordAgain", DataType::WeakPointer),
        ("weakInstance", DataType::WeakPointer),
        ("weakInstanceAgain", DataType::WeakPointer),
        ("nullStrong", DataType::StrongPointer),
        ("nullWeak", DataType::WeakPointer),
        ("nullReference", DataType::Reference),
    ];
    assert_eq!(
        tree.error_count(),
        0,
        "all actual references resolve without source error"
    );
    assert_eq!(
        tree.fields().len(),
        expected.len(),
        "exact original ordered field count"
    );
    for (field, (name, storage)) in tree.fields().iter().zip(expected) {
        assert_eq!(field.name(), name, "actual original declaration order");
        assert_eq!(
            field.storage().data_type(),
            storage,
            "actual Reference/Weak/Strong distinction"
        );
        assert!(
            !field.storage().is_array(),
            "each original caller is a separate scalar field"
        );
    }
}

fn check_owned(tree: &RecordTree<'_>) -> TestResult {
    let Some(Value::Struct(owned)) = tree.field("owned") else {
        return Err("actual first original owned instance".into());
    };
    assert_eq!(
        owned.type_name(),
        "Owned",
        "the original instance is not its owner root type"
    );
    assert_eq!(
        owned.fields().len(),
        1,
        "the only nested value fixes independent source ordinals"
    );
    assert_eq!(
        owned.field("number"),
        Some(&Value::Int32(11)),
        "actual owned payload"
    );
    for name in ["nullStrong", "nullWeak", "nullReference"] {
        assert_eq!(
            tree.field(name),
            Some(&Value::Null),
            "genuine original null {name}"
        );
    }
    Ok(())
}

fn check_record_references(owner: &Record<'_>, tree: &RecordTree<'_>) -> TestResult {
    let stored = reference(tree, "first")?;
    let weak = reference(tree, "weakRecord")?;
    assert_eq!(
        stored, weak,
        "equal visible target descriptor does not determine stored type"
    );
    for name in ["first", "second", "weakRecord", "weakRecordAgain"] {
        let reference = reference(tree, name)?;
        check_record_target(owner, reference);
    }
    Ok(())
}

fn check_record_target(owner: &Record<'_>, reference: &Reference<'_>) {
    let target = reference.original_target();
    assert!(
        target.shares_source_with(owner),
        "actual original Forge admission"
    );
    assert_eq!(
        target.record_index(),
        1,
        "the actual selected physical target"
    );
    assert_eq!(
        target.record_guid().to_string(),
        fixture::TARGET,
        "actual admitted target GUID"
    );
    assert_eq!(
        target.record_path(),
        fixture::TARGET_PATH,
        "actual target path"
    );
    assert_eq!(
        target.record_type(),
        Some("PoolFilterRecord"),
        "Record applicability"
    );
    assert!(
        target.instance_location().is_none() && target.instance_positions().is_none(),
        "equal location-none Reference/Weak descriptors are original Record targets"
    );
}

fn check_instance_references(owner: &Record<'_>, tree: &RecordTree<'_>) -> TestResult {
    for name in ["weakInstance", "weakInstanceAgain"] {
        let reference = reference(tree, name)?;
        let target = reference.original_target();
        assert!(
            target.shares_source_with(owner),
            "actual original instance admission"
        );
        assert_eq!(target.record_index(), 0, "the actual owning Record");
        assert_eq!(
            target.record_guid().to_string(),
            fixture::OWNER,
            "actual owning Record GUID"
        );
        assert_eq!(
            target.record_path(),
            fixture::OWNER_PATH,
            "actual owning Record path"
        );
        assert_eq!(
            reference.type_name(),
            "Owned",
            "actual instance type differs from owner root type"
        );
        assert_eq!(
            target.record_type(),
            None,
            "Record type is genuinely inapplicable"
        );
        assert_eq!(
            target.instance_location().map(Location::as_str),
            Some("owned"),
            "actual original named-field location"
        );
        assert_eq!(
            target.instance_positions(),
            Some(&[0][..]),
            "actual original field position receipt"
        );
    }
    Ok(())
}

fn reference<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    name: &str,
) -> TestResult<&'tree Reference<'source>> {
    let Some(Value::Reference(reference)) = tree.field(name) else {
        return Err(format!("actual original Reference at {name}").into());
    };
    Ok(reference)
}
