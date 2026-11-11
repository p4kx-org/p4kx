// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataType, Guid, Record, Struct};

use crate::layout_field_fixture::{self as fixture, BASE, DERIVED, TestResult};

pub(crate) fn record<'source>(
    forge: &'source DataForge<'source>,
    own: usize,
    padding: usize,
) -> TestResult<Record<'source>> {
    assert_eq!(
        forge.struct_count(),
        2,
        "the two actual original definitions"
    );
    assert_eq!(
        forge.property_count(),
        2 * own,
        "the physical property table cardinality is retained without truncation"
    );
    assert_eq!(forge.record_count(), 1, "one physical original Record");
    for (index, structure) in forge.structs().enumerate() {
        definition(structure, index, own, padding)?;
    }
    let record = forge.record(0).ok_or("the original descendant Record")?;
    assert_eq!(
        record.guid(),
        Guid::from_stored([0x77; 16]),
        "the actual original GUID bytes"
    );
    assert_eq!(
        record.struct_type()?.index(),
        1,
        "the actual u32 Record type selects the descendant"
    );
    assert_eq!(
        record.name()?,
        "WideDerived.Source",
        "the actual original Record name"
    );
    assert_eq!(
        record.record_path()?,
        "source/wide-fields",
        "the actual original source path"
    );
    Ok(record)
}

fn definition(structure: Struct<'_>, index: usize, own: usize, padding: usize) -> TestResult {
    assert_eq!(
        structure.index(),
        u32::try_from(index)?,
        "actual definition position"
    );
    assert_eq!(
        structure.name()?,
        if index == 0 { BASE } else { DERIVED },
        "actual definition name"
    );
    assert_eq!(
        structure.instance_count(),
        u32::try_from(index)?,
        "only the descendant has an original instance"
    );
    let expected_size = if index == 0 { own } else { 2 * own + padding };
    assert_eq!(
        structure.size(),
        u32::try_from(expected_size)?,
        "the original declared instance width"
    );
    let parent = structure.parent().map(|parent| parent.index());
    assert_eq!(
        parent,
        if index == 0 { None } else { Some(0) },
        "the exact acyclic original parent link"
    );
    properties(structure, index * own, own)
}

fn properties(structure: Struct<'_>, first: usize, own: usize) -> TestResult {
    let mut count = 0;
    for (ordinal, property) in structure.properties().enumerate() {
        assert_eq!(
            property.name()?,
            fixture::field_name(first + ordinal, own),
            "original own declaration order"
        );
        assert_eq!(
            property.data_type()?,
            DataType::UInt8,
            "every original property is one scalar byte"
        );
        assert_eq!(property.conversion(), 0, "no array or conversion flag");
        assert!(!property.is_array(), "the actual declaration is scalar");
        count += 1;
    }
    assert_eq!(
        count, own,
        "every original own declaration remains addressable"
    );
    Ok(())
}
