// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, RecordText, Value as Raw, ValueError};
use p4kx_testkit::DataForgeBuilder;

use crate::category_lists_v3_fixture as fixture;
use crate::commodity_v3_fixture::{self as common, TestResult};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Null,
    Reference {
        guid: String,
        index: u32,
        kind: String,
        path: String,
    },
    Dangling(String),
}

pub(crate) fn declaration(source: &DataForgeBuilder) -> TestResult {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let listing = forge
        .structs()
        .find(|entry| entry.name() == Ok(fixture::TYPE))
        .ok_or("original list declaration")?;
    let fields = listing.properties().collect::<Vec<_>>();
    assert_eq!(
        fields.len(),
        1,
        "the actual list declaration owns one field"
    );
    let field = fields.first().ok_or("original categories declaration")?;
    assert_eq!(field.name()?, "categories", "original authored field name");
    assert_eq!(
        field.data_type()?,
        DataType::Reference,
        "original element contract"
    );
    assert!(field.is_array(), "the original declaration is an array");
    let target = field.target_struct().ok_or("original declared target")?;
    assert_eq!(
        target.name()?,
        fixture::CATEGORY_TYPE,
        "the declared category target survives"
    );
    assert_ne!(
        target.index(),
        0,
        "the fixture's target is not the untyped index-zero convention"
    );
    Ok(())
}

pub(crate) fn header(
    source: &DataForgeBuilder,
    index: u32,
    guid: u128,
    name: &str,
    file: &str,
    tag: &str,
) -> TestResult {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(index).ok_or("original physical list")?;
    assert_eq!(record.index(), index, "original physical list index");
    assert_eq!(
        record.guid().to_string(),
        common::guid(guid),
        "original nominal identity"
    );
    assert_eq!(
        record.type_name()?,
        fixture::TYPE,
        "the list is not a category definition"
    );
    assert_eq!(
        record.stored_name()?,
        RecordText::Present(name),
        "raw source name"
    );
    assert_eq!(record.stored_file_name()?, file, "exact source filename");
    assert_eq!(
        record.stored_tag()?,
        RecordText::Present(tag),
        "raw source metadata tag"
    );
    Ok(())
}

pub(crate) fn primary(source: &DataForgeBuilder) -> TestResult {
    declaration(source)?;
    header(
        source,
        0,
        fixture::LIST,
        "BlueprintCategoryDatabaseRecord.Primary",
        fixture::LIST_FILE,
        "PrimaryDomain",
    )
}

pub(crate) fn slots(source: &DataForgeBuilder, index: u32) -> TestResult<Vec<Slot>> {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(index).ok_or("original list Record")?.tree()?;
    let Some(Raw::Array(values)) = tree.root().field("categories") else {
        return Err("the original categories field must really be an array".into());
    };
    values.iter().map(slot).collect()
}

fn slot(value: &Raw<'_>) -> TestResult<Slot> {
    Ok(match value {
        Raw::Null => Slot::Null,
        Raw::Reference(target) => Slot::Reference {
            guid: target.guid().to_string(),
            index: target.record_index(),
            kind: target.type_name().to_owned(),
            path: target.record_path().to_owned(),
        },
        Raw::Error(ValueError::DanglingReference { guid }) => Slot::Dangling(guid.to_string()),
        _ => return Err("unexpected original category-list slot kind".into()),
    })
}

pub(crate) fn reference(guid: u128, index: u32, kind: &str, path: &str) -> Slot {
    Slot::Reference {
        guid: common::guid(guid),
        index,
        kind: kind.to_owned(),
        path: path.to_owned(),
    }
}

pub(crate) fn ordered(source: &DataForgeBuilder, errors: bool) -> TestResult {
    primary(source)?;
    let mut expected = vec![
        reference(
            fixture::BETA,
            3,
            fixture::CATEGORY_TYPE,
            &format!("{}Beta", fixture::CATEGORY_FOLDER),
        ),
        Slot::Null,
        reference(
            fixture::ALPHA,
            1,
            fixture::CATEGORY_TYPE,
            &format!("{}Alpha", fixture::CATEGORY_FOLDER),
        ),
        reference(
            fixture::BETA,
            3,
            fixture::CATEGORY_TYPE,
            &format!("{}Beta", fixture::CATEGORY_FOLDER),
        ),
    ];
    if errors {
        expected.push(Slot::Dangling(common::guid(fixture::MISSING)));
        expected.push(reference(fixture::OTHER, 2, "Other", "synthetic/foreign"));
    }
    assert_eq!(
        slots(source, 0)?,
        expected,
        "actual source order/null/repeated/dangling/foreign slots"
    );
    Ok(())
}
