// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, DataForgeEntry, DataType, Field, RecordTree, Value, ValueError};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};

pub(crate) type Checked<T = ()> = Result<T, Box<dyn Error>>;

#[derive(Clone, Copy)]
pub(crate) enum Encoding {
    Stored,
    Zstd,
}

#[derive(Clone, Copy)]
pub(crate) enum Case {
    Finite,
    Nonfinite,
    Unreadable,
}

pub(crate) struct Fixture {
    pub(crate) archive: Archive,
    _directory: tempfile::TempDir,
}

pub(crate) fn archive(encoding: Encoding, case: Case) -> Checked<Fixture> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("loot-numbers.p4k");
    let bytes = database(case)?;
    let entry = match encoding {
        Encoding::Stored => TestEntry::stored(r"Data\Game2.dcb", bytes),
        Encoding::Zstd => TestEntry::zstd(r"Data\Game2.dcb", bytes),
    };
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    Ok(Fixture {
        archive: Archive::open_with_key(path, None)?,
        _directory: directory,
    })
}

fn database(case: Case) -> Checked<Vec<u8>> {
    const ORIGINAL: &[u8] = b"duplicatE\0";
    const REPLACEMENT: &[u8] = b"duplicate\0";
    let mut bytes = schema().record(record(case)).build();
    let positions: Vec<_> = bytes
        .windows(ORIGINAL.len())
        .enumerate()
        .filter_map(|(position, window)| (window == ORIGINAL).then_some(position))
        .collect();
    let [position] = positions.as_slice() else {
        return Err("one original second synthetic declaration name".into());
    };
    assert_eq!(
        ORIGINAL.len(),
        REPLACEMENT.len(),
        "original offsets remain unchanged"
    );
    bytes
        .get_mut(*position..*position + ORIGINAL.len())
        .ok_or("original stored declaration name")?
        .copy_from_slice(REPLACEMENT);
    Ok(bytes)
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("PoolFilter"))
        .structure(
            TestStruct::new("PoolFilterRecord")
                .field("filter", TestType::StrongPointer("PoolFilter".to_owned()))
                .field("single", TestType::Single)
                .field("double", TestType::Double)
                .array("samples", TestType::Double)
                .field("duplicate", TestType::Single)
                .field("duplicatE", TestType::Single)
                .field("corrupt", TestType::Bool),
        )
}

fn record(case: Case) -> TestRecord {
    let single = if matches!(case, Case::Finite) {
        -0.0
    } else {
        f32::from_bits(0x7fc0_1234)
    };
    let double = if matches!(case, Case::Finite) {
        -0.0
    } else {
        f64::NEG_INFINITY
    };
    let first = if matches!(case, Case::Finite) {
        3.5
    } else {
        f64::from_bits(0x7ff8_0000_0000_0042)
    };
    TestRecord::new(
        0xe920_0000_0000_4000_8000_0000_0000_0001,
        "PoolFilterRecord",
        "PoolFilterRecord.Numbers",
        "libs/foundry/records/lootgeneration/filters/numbers.xml",
    )
    .value("single", TestValue::Single(single))
    .value("double", TestValue::Double(double))
    .value(
        "samples",
        TestValue::Array(vec![
            TestValue::Double(first),
            TestValue::Double(double),
            TestValue::Double(0.0),
        ]),
    )
    .value("duplicate", TestValue::Single(single))
    .value("duplicatE", TestValue::Single(single))
    .value(
        "corrupt",
        if matches!(case, Case::Unreadable) {
            TestValue::Raw(vec![0x81])
        } else {
            TestValue::Bool(false)
        },
    )
}

pub(crate) fn check_source(archive: &Archive, case: Case) -> Checked {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let record = forge.record(0).ok_or("actual numeric Loot source")?;
    let tree = record.tree()?;
    assert!(tree.is_from(&record), "actual original tree owner");
    assert_eq!(record.index(), 0, "the original physical Record position");
    check_declarations(tree.fields());
    check_scalars(tree.fields(), case)?;
    check_array(tree.fields(), case)?;
    check_corrupt(&tree, case)
}

fn check_declarations(fields: &[Field<'_>]) {
    let expected = [
        ("filter", DataType::StrongPointer, false),
        ("single", DataType::Single, false),
        ("double", DataType::Double, false),
        ("samples", DataType::Double, true),
        ("duplicate", DataType::Single, false),
        ("duplicate", DataType::Single, false),
        ("corrupt", DataType::Bool, false),
    ];
    assert_eq!(
        fields.len(),
        expected.len(),
        "including both actual same-name declarations"
    );
    for (field, (name, data_type, is_array)) in fields.iter().zip(expected) {
        assert_eq!(field.name(), name, "original ordered declaration name");
        assert_eq!(
            field.storage().data_type(),
            data_type,
            "actual decoder element storage"
        );
        assert_eq!(
            field.storage().is_array(),
            is_array,
            "actual decoder array flag"
        );
    }
}

fn check_scalars(fields: &[Field<'_>], case: Case) -> Checked {
    assert!(
        matches!(fields.first().map(Field::value), Some(Value::Null)),
        "actual null filter"
    );
    let single_bits = if matches!(case, Case::Finite) {
        0x8000_0000
    } else {
        0x7fc0_1234
    };
    for index in [1, 4, 5] {
        let field = fields.get(index).ok_or("original Single occurrence")?;
        assert!(
            matches!(field.value(), Value::Single(value) if value.to_bits()==single_bits),
            "exact Single bits at source {index}"
        );
    }
    let double_bits = if matches!(case, Case::Finite) {
        0x8000_0000_0000_0000
    } else {
        0xfff0_0000_0000_0000
    };
    assert!(
        matches!(fields.get(2).map(Field::value), Some(Value::Double(value)) if value.to_bits()==double_bits),
        "exact original Double storage bits"
    );
    Ok(())
}

fn check_array(fields: &[Field<'_>], case: Case) -> Checked {
    let Some(Value::Array(samples)) = fields.get(3).map(Field::value) else {
        return Err("original Double array".into());
    };
    assert_eq!(samples.len(), 3, "three original array slots");
    let first_bits = if matches!(case, Case::Finite) {
        3.5_f64.to_bits()
    } else {
        0x7ff8_0000_0000_0042
    };
    let double_bits = if matches!(case, Case::Finite) {
        0x8000_0000_0000_0000
    } else {
        0xfff0_0000_0000_0000
    };
    for (value, bits) in samples.iter().zip([first_bits, double_bits, 0]) {
        assert!(
            matches!(value, Value::Double(value) if value.to_bits()==bits),
            "original array storage bits"
        );
    }
    Ok(())
}

fn check_corrupt(tree: &RecordTree<'_>, case: Case) -> Checked {
    let corrupt = tree.fields().get(6).ok_or("original Bool field")?.value();
    if matches!(case, Case::Unreadable) {
        assert!(
            matches!(
                corrupt,
                Value::Error(ValueError::InvalidBool { byte: 0x81 })
            ),
            "actual independent unreadable token"
        );
        assert_eq!(
            tree.error_count(),
            1,
            "only the independent malformed Bool is unreadable"
        );
    } else {
        assert!(
            matches!(corrupt, Value::Bool(false)),
            "the readable control retains false"
        );
        assert_eq!(
            tree.error_count(),
            0,
            "every control source occurrence is readable"
        );
    }
    Ok(())
}
