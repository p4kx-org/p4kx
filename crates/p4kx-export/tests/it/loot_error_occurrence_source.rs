// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeEntry, DataType, RecordTree, SourceEncryption, Value, ValueError};

use crate::loot_error_occurrence_fixture::{
    Case, Checked, DANGLING_TEXT, Fixture, GUID_TEXT, Storage,
};

pub(crate) fn check(input: &Fixture, case: Case, storage: Storage) -> Checked {
    let entry = DataForgeEntry::read(&input.archive)?;
    assert!(
        entry.origin().matches_archive(&input.archive),
        "actual physical Archive admission"
    );
    assert_eq!(
        entry.origin().encryption(),
        if storage.encrypted() {
            SourceEncryption::Encrypted
        } else {
            SourceEncryption::Unencrypted
        },
        "actual stored source precondition for every origin marker"
    );
    let forge = entry.open()?;
    assert_eq!(
        forge.records().len(),
        1,
        "one genuine physical source Record"
    );
    let record = forge.record(0).ok_or("original Record")?;
    let tree = record.tree()?;
    assert!(tree.is_from(&record), "actual canonical Tree/Record owner");
    assert_eq!(
        record.index(),
        0,
        "the actual original physical Record position"
    );
    assert_eq!(
        record.guid().to_string(),
        GUID_TEXT,
        "original synthetic identity"
    );
    match case {
        Case::RepeatedBool => check_repeated(&tree),
        Case::ClassAlias => check_class_alias(&tree),
        Case::Dangling => check_dangling(&tree),
    }
}

fn check_repeated(tree: &RecordTree<'_>) -> Checked {
    assert_eq!(
        tree.error_count(),
        12,
        "all twelve original errors precede production"
    );
    let field = tree.fields().first().ok_or("actual errors declaration")?;
    assert_eq!(tree.fields().len(), 1, "one original root declaration");
    assert_eq!(field.name(), "errors", "actual original array field name");
    assert_eq!(
        field.storage().data_type(),
        DataType::Bool,
        "actual Bool pool declaration"
    );
    assert!(field.storage().is_array(), "actual array declaration");
    let Value::Array(values) = field.value() else {
        return Err("actual original error array".into());
    };
    assert_eq!(values.len(), 12, "no original slot is missing");
    for value in values {
        assert!(
            matches!(value, Value::Error(ValueError::InvalidBool { byte: 0x81 })),
            "each original retained byte is 129"
        );
    }
    Ok(())
}

fn check_class_alias(tree: &RecordTree<'_>) -> Checked {
    check_class_declarations(tree);
    let Some(Value::Array(left)) = tree.field("left") else {
        return Err("actual original first class array".into());
    };
    assert_eq!(left.len(), 1, "one first original instance");
    let Some(Value::Struct(value)) = left.first() else {
        return Err("original owned first instance".into());
    };
    assert_eq!(value.type_name(), "Shared", "actual original owned class");
    assert_eq!(
        value.fields().len(),
        1,
        "one original field fixes independent DFS positions"
    );
    assert_eq!(
        value.field("number"),
        Some(&Value::Int32(11)),
        "first ownership genuinely reads its data"
    );
    let Some(Value::Array(right)) = tree.field("right") else {
        return Err("actual original aliased class array".into());
    };
    assert_eq!(right.len(), 1, "one duplicate source mapping");
    assert!(
        matches!(
            right.first(),
            Some(Value::Error(ValueError::OwnedTwice {
                struct_index: 1,
                instance_index: 0,
            }))
        ),
        "actual original Class mapping, not fake pointer words"
    );
    Ok(())
}

fn check_class_declarations(tree: &RecordTree<'_>) {
    assert_eq!(tree.fields().len(), 2, "two genuine array declarations");
    assert_eq!(
        tree.error_count(),
        1,
        "one genuine duplicate ownership failure"
    );
    for (field, name) in tree.fields().iter().zip(["left", "right"]) {
        assert_eq!(field.name(), name, "original array declaration order");
        assert_eq!(
            field.storage().data_type(),
            DataType::Class,
            "Class mapping is not a stored pointer pool"
        );
        assert!(
            field.storage().is_array(),
            "both original mappings are arrays"
        );
        assert_eq!(
            field.storage().target(),
            1,
            "original nonzero declaration target"
        );
    }
}

fn check_dangling(tree: &RecordTree<'_>) -> Checked {
    assert_eq!(tree.error_count(), 1, "one original unresolved GUID");
    let Some(Value::Error(ValueError::DanglingReference { guid })) = tree.field("broken") else {
        return Err("actual canonical dangling Reference".into());
    };
    assert_eq!(
        guid.to_string(),
        DANGLING_TEXT,
        "exact authored GUID, not a resolved target descriptor"
    );
    Ok(())
}
