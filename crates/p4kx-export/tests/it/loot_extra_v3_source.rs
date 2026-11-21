// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, DataForgeEntry, DataType, Record, RecordTree, SourceEncryption, StructValue, Value,
    ValueError,
};

use crate::loot_extra_v3_fixture::{self as fixture, Case, Checked};

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Step {
    Field(usize),
    Element(usize),
}

pub(crate) struct Original {
    paths: Vec<Vec<Step>>,
    pub(crate) filters_target: u16,
}

impl Original {
    pub(crate) const fn count(&self) -> usize {
        self.paths.len()
    }

    pub(crate) fn ordinal(&self, path: &[Step]) -> Checked<u64> {
        let position = self
            .paths
            .iter()
            .position(|candidate| candidate == path)
            .ok_or("actual original source occurrence")?;
        Ok(u64::try_from(position)?)
    }
}

pub(crate) fn check(archive: &Archive, case: Case) -> Checked<Original> {
    let input = DataForgeEntry::read(archive)?;
    check_input(&input, archive);
    let forge = input.open()?;
    assert_eq!(
        forge.records().len(),
        1,
        "exactly one actual original physical Record"
    );
    let record = forge.record(0).ok_or("actual original physical Record")?;
    check_record(&record)?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "actual immutable tree/Record ownership is proved before JSON"
    );
    let errors = usize::from(matches!(case, Case::Holes));
    assert_eq!(
        tree.error_count(),
        errors,
        "only the intended genuine malformed pointer is unreadable"
    );
    check_root_values(&tree);
    let filter = structure(tree.root(), "filter")?;
    check_filter(filter, case)?;
    let filters_target = filter
        .fields()
        .first()
        .ok_or("original first filter field")?
        .storage()
        .target();
    assert_eq!(
        filters_target, 2,
        "the actual pointer declaration names its nonzero concrete target"
    );
    let original = Original {
        paths: enumerate(tree.root()),
        filters_target,
    };
    check_positions(&original, errors)?;
    Ok(original)
}

fn check_input(input: &DataForgeEntry, archive: &Archive) {
    assert_eq!(
        input.path(),
        "Data\\Game2.dcb",
        "the actual selected input Entry"
    );
    assert!(
        input.origin().matches_archive(archive),
        "original DataForge input belongs to the actual Archive"
    );
    assert_eq!(
        input.origin().encryption(),
        SourceEncryption::Unencrypted,
        "false per-datum markers have an actual plain source precondition"
    );
}

fn check_record(record: &Record<'_>) -> Checked {
    assert_eq!(record.index(), 0, "the physical original Record ordinal");
    assert_eq!(
        record.guid().to_string(),
        fixture::GUID_TEXT,
        "actual original identity"
    );
    assert_eq!(
        record.type_name()?,
        "PoolFilterRecord",
        "the existing owning kind is genuine"
    );
    assert_eq!(
        record.record_path()?,
        fixture::PATH,
        "the original source is inside the ordinary filter family"
    );
    check_declarations(record)?;
    Ok(())
}

fn check_positions(original: &Original, errors: usize) -> Checked {
    assert_eq!(
        original.count(),
        if errors == 0 { 23 } else { 25 },
        "complete actual DFS counts every occurrence, including null/error"
    );
    assert_eq!(
        original.ordinal(&[Step::Field(1)])?,
        if errors == 0 { 19 } else { 21 },
        "worked original wide-integer position is independent from output arena indexes"
    );
    Ok(())
}

fn check_declarations(record: &Record<'_>) -> Checked {
    let declarations: Vec<_> = record
        .struct_type()?
        .properties()
        .map(|property| Ok((property.name()?, property.data_type()?, property.is_array())))
        .collect::<Checked<_>>()?;
    assert_eq!(
        declarations,
        [
            ("filter", DataType::StrongPointer, false),
            ("rootExtra", DataType::UInt64, false),
            ("rootFlag", DataType::Bool, false),
            ("rootEmpty", DataType::String, false),
            ("rootLocale", DataType::Locale, false),
        ],
        "all actual root declaration names/types/order are independently read"
    );
    Ok(())
}

fn check_root_values(tree: &RecordTree<'_>) {
    assert_eq!(
        tree.field("rootExtra"),
        Some(&Value::UInt64(u64::MAX)),
        "actual exact wide integer"
    );
    assert_eq!(
        tree.field("rootFlag"),
        Some(&Value::Bool(false)),
        "actual false is not absent"
    );
    assert_eq!(
        tree.field("rootEmpty"),
        Some(&Value::String("")),
        "actual empty string is present"
    );
    assert_eq!(
        tree.field("rootLocale"),
        Some(&Value::Locale(fixture::LOCALE)),
        "actual Locale retains its original key"
    );
}

fn check_filter(filter: &StructValue<'_>, case: Case) -> Checked {
    assert_eq!(
        filter.type_name(),
        "PoolFilter_Custom",
        "unknown concrete source type stays real"
    );
    let fields: Vec<_> = filter
        .fields()
        .iter()
        .map(|field| {
            (
                field.name(),
                field.storage().data_type(),
                field.storage().is_array(),
            )
        })
        .collect();
    assert_eq!(
        fields,
        [
            ("filters", DataType::StrongPointer, true),
            ("extraArray", DataType::Int32, true),
            ("extraObject", DataType::Class, false)
        ],
        "all original nested declarations survive decoding"
    );
    check_source_slots(filter, case)?;
    assert_eq!(
        filter.field("extraArray"),
        Some(&Value::Array(
            vec![Value::Int32(11), Value::Int32(-22), Value::Int32(0)].into_boxed_slice()
        )),
        "all exact original numeric array slots"
    );
    let extra = structure(filter, "extraObject")?;
    assert_eq!(
        extra.field("preserveName"),
        Some(&Value::String(fixture::LITERAL)),
        "literal whitespace/backslash/format content stays inert"
    );
    assert_eq!(
        extra.field("preserveFalse"),
        Some(&Value::Bool(false)),
        "nested false is original data"
    );
    Ok(())
}

fn check_source_slots(filter: &StructValue<'_>, case: Case) -> Checked {
    let Some(Value::Array(slots)) = filter.field("filters") else {
        return Err("original pointer-array slots".into());
    };
    let expected = if matches!(case, Case::Holes) { 4 } else { 2 };
    assert_eq!(
        slots.len(),
        expected,
        "actual original array cardinality is not filtered"
    );
    check_instance(slots.first().ok_or("original first slot")?, "first", -7)?;
    check_instance(slots.last().ok_or("original final slot")?, "last", -8)?;
    if matches!(case, Case::Holes) {
        assert_eq!(
            slots.get(1),
            Some(&Value::Null),
            "genuine null pointer slot"
        );
        assert_eq!(
            slots.get(2),
            Some(&Value::Error(ValueError::InvalidPointer {
                struct_index: 77,
                instance_index: 99
            })),
            "actual malformed pointer words become their precise decoder error"
        );
    }
    Ok(())
}

fn check_instance(value: &Value<'_>, name: &str, number: i32) -> Checked {
    let Value::Struct(instance) = value else {
        return Err("actual original readable instance".into());
    };
    assert_eq!(
        instance.field("name"),
        Some(&Value::String(name)),
        "actual ordered instance label"
    );
    assert_eq!(
        instance.field("mode"),
        Some(&Value::String("Cumulative")),
        "actual designer mode"
    );
    let leaf = structure(instance, "filter")?;
    assert_eq!(
        leaf.field("leafExtra"),
        Some(&Value::Int32(number)),
        "the exact original nested extra value"
    );
    Ok(())
}

fn structure<'tree, 'source>(
    owner: &'tree StructValue<'source>,
    name: &str,
) -> Checked<&'tree StructValue<'source>> {
    match owner.field(name) {
        Some(Value::Struct(value)) => Ok(value),
        _ => Err(format!("original {name} is an actual owned struct").into()),
    }
}

fn enumerate(root: &StructValue<'_>) -> Vec<Vec<Step>> {
    let mut pending: Vec<_> = root
        .fields()
        .iter()
        .enumerate()
        .rev()
        .map(|(index, field)| (field.value(), vec![Step::Field(index)]))
        .collect();
    let mut paths = Vec::new();
    while let Some((value, path)) = pending.pop() {
        paths.push(path.clone());
        match value {
            Value::Struct(structure) => {
                for (index, field) in structure.fields().iter().enumerate().rev() {
                    let mut next = path.clone();
                    next.push(Step::Field(index));
                    pending.push((field.value(), next));
                }
            }
            Value::Array(values) => {
                for (index, value) in values.iter().enumerate().rev() {
                    let mut next = path.clone();
                    next.push(Step::Element(index));
                    pending.push((value, next));
                }
            }
            _ => {}
        }
    }
    paths
}
