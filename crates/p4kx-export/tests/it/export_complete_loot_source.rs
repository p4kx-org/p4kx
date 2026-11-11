// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, DataForgeEntry, StructValue, Value};

use crate::export_complete_loot_fixture as fixture;
use crate::starmap_fixture;

pub(crate) fn check(archive: &Archive, layers: usize) -> Result<(), Box<dyn Error>> {
    let loaded = DataForgeEntry::read(archive)?;
    let forge = loaded.open()?;
    let record = forge
        .record_by_guid(starmap_fixture::guid_text(fixture::CHAIN).parse()?)
        .ok_or("original filter chain")?;
    let tree = record.tree()?;
    assert_eq!(
        tree.error_count(),
        0,
        "the complete original deep tree is readable"
    );
    let mut filter = child(tree.root(), "filter")?;
    for ordinal in 0..layers {
        assert_eq!(
            filter.type_name(),
            "PoolFilter_Sequence",
            "original sequence {ordinal}"
        );
        let Some(Value::Array(slots)) = filter.field("filters") else {
            return Err("original sequence array".into());
        };
        let [Value::Struct(instance)] = slots.as_ref() else {
            return Err("one original ordered filter instance".into());
        };
        assert_eq!(
            instance.field("name").and_then(text),
            Some(format!("step-{ordinal}").as_str()),
            "the actual original slot label precedes output assertions"
        );
        filter = child(instance, "filter")?;
    }
    assert_eq!(
        filter.type_name(),
        "PoolFilter_RecordRef",
        "the deep original leaf is retained"
    );
    let Some(Value::Reference(reference)) = filter.field("filterRecord") else {
        return Err("original terminal filter Reference".into());
    };
    assert_eq!(
        reference.guid().to_string(),
        starmap_fixture::guid_text(fixture::TARGET),
        "the terminal original Reference resolves to its actual target"
    );
    assert_eq!(
        reference.type_name(),
        "PoolFilterRecord",
        "original target header type"
    );
    Ok(())
}

fn child<'tree, 'source>(
    structure: &'tree StructValue<'source>,
    field: &str,
) -> Result<&'tree StructValue<'source>, Box<dyn Error>> {
    if let Some(Value::Struct(child)) = structure.field(field) {
        Ok(child)
    } else {
        Err(format!("original {field} is an owned struct").into())
    }
}

const fn text<'source>(value: &Value<'source>) -> Option<&'source str> {
    if let Value::String(text) = value {
        Some(*text)
    } else {
        None
    }
}
