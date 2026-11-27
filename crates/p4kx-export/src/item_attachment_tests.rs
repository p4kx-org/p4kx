// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{Archive, Record, SourceInventory, SourceValueState, SourcedValue};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::{AttachmentDefinition, COMPONENT, DEFINITION, Observed, TYPE, first};
use crate::build::original_input::test_support;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::catalog_item_components;
use crate::catalog_v3::catalog_item_components::item_definition::{
    ItemAttachmentPresentation, ItemDefinition, ItemDefinitionComponentKind,
};
use crate::catalog_v3::catalog_item_components::selection::ItemSourceSelection;
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;
use crate::texts::TextSources;
use crate::{CatalogError, ExportAccountingError, Kind};

const ENTITY: &str = "EntityClassDefinition";
const BASE_COMPONENT: &str = "DataForgeComponentParams";
const ATTACHMENT: &str = "SItemDefinition";

#[test]
fn paint_selectors_use_first_physical_attachment_even_when_its_type_is_unreadable()
-> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    for storage in [
        TestType::Class(ATTACHMENT.to_owned()),
        TestType::StrongPointer(ATTACHMENT.to_owned()),
    ] {
        let path = folder.path().join("paint-attachment.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::stored(
                "Data/Game2.dcb",
                database(storage).build(),
            ))
            .write_to(&path)?;
        let archive = Archive::open(&path)?;
        let sources = TextSources::read_item_languages(&archive, &[])?;
        test_support::with_archive_projection(&archive, &sources, |forge, projection| {
            let projection = projection.for_kind(Kind::Items);
            for (index, expected) in [(0, Some("Paints")), (1, None)] {
                let record = forge
                    .record(index)
                    .ok_or(ExportAccountingError::MissingFieldInspection)?;
                assert_attachment(&record, &projection, expected)?;
            }
            Ok::<_, CatalogError>(())
        })?;
    }
    Ok(())
}

fn assert_attachment<'source>(
    record: &Record<'source>,
    projection: &CatalogProjection<'_, 'source>,
    expected: Option<&str>,
) -> Result<(), CatalogError> {
    let tree = record.tree().map_err(CatalogError::DataForge)?;
    assert_eq!(
        crate::item::is_paint(&tree),
        expected.is_some(),
        "raw admission uses only the first attachment",
    );
    let input = projection.original();
    let admission = RecordObservation::capture(input, record)?;
    catalog_item_components::read(
        &tree,
        projection,
        &admission,
        None,
        |definition, originals, _| {
            let selection = ItemSourceSelection::read(&tree, originals)?;
            let ordinal = selection.attachment_component_ordinal();
            assert_eq!(
                ordinal.value(),
                Some(&0),
                "first physical occurrence is selected"
            );
            assert!(
                ordinal.origin().source_set_complete(),
                "the array prefix is readable"
            );
            let (value, _) = first(&tree).ok_or(ExportAccountingError::MissingFieldInspection)?;
            let capture = TemplateFieldCapture::new(originals, input, &admission)?;
            let numbers = RefCell::new(Vec::new());
            let source =
                Observed::new(originals, input, originals.fact(value)?, &capture, &numbers);
            let mut notes = Vec::new();
            let focused = AttachmentDefinition::read(source, &mut notes, |fields, notes| {
                Ok((
                    fields.item_type(notes)?,
                    fields.tags(notes)?,
                    fields.required_tags(notes)?,
                ))
            })?;
            let (item_type, tags, required_tags) = focused
                .value()
                .ok_or(ExportAccountingError::MissingFieldInspection)?;
            let full = first_presentation(definition)?;
            let inventory = projection.origins().uses.freeze()?;
            assert_label(item_type, full.item_type(), expected, &inventory);
            assert_label(tags, full.tags(), Some("Paint_First Child"), &inventory);
            assert_label(
                required_tags,
                full.required_tags(),
                Some("Required_First"),
                &inventory,
            );
            Ok(())
        },
    )?;
    Ok(())
}

fn first_presentation<'row, 'source>(
    definition: &'row ItemDefinition<'source>,
) -> Result<&'row ItemAttachmentPresentation<'source>, CatalogError> {
    let components = definition
        .components()
        .value()
        .ok_or(ExportAccountingError::MissingFieldInspection)?;
    let [first, _] = components.as_slice() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let ItemDefinitionComponentKind::Attachment(full) = first.body() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    full.value()
        .map(Box::as_ref)
        .ok_or_else(|| ExportAccountingError::MissingFieldInspection.into())
}

fn assert_label(
    focused: &SourcedValue<&str>,
    normal: &SourcedValue<&str>,
    expected: Option<&str>,
    inventory: &SourceInventory,
) {
    assert_eq!(
        focused, normal,
        "focused reads preserve Normal value and source evidence"
    );
    assert_eq!(
        focused.value().copied(),
        expected,
        "only the first attachment supplies labels"
    );
    let state = if expected.is_some() {
        SourceValueState::Present
    } else {
        SourceValueState::Unreadable
    };
    assert_eq!(
        focused.state(),
        state,
        "corrupt text retains its unreadable state"
    );
    assert_eq!(
        focused.origin().source_set_complete(),
        expected.is_some(),
        "the unreadable label cannot yield complete source evidence",
    );
    let references = focused.origin().source_uses_refs();
    assert!(
        !references.is_empty(),
        "the original selectors are retained"
    );
    assert!(
        references.iter().all(|reference| {
            inventory
                .iter_uses()
                .any(|observed| observed.id() == reference)
        }),
        "every field contribution resolves in the original source inventory",
    );
}

fn database(storage: TestType) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(ENTITY).array(
            "Components",
            TestType::StrongPointer(BASE_COMPONENT.to_owned()),
        ))
        .structure(TestStruct::new(BASE_COMPONENT))
        .structure(
            TestStruct::new(COMPONENT)
                .parent(BASE_COMPONENT)
                .field(DEFINITION, storage),
        )
        .structure(
            TestStruct::new(ATTACHMENT)
                .field(TYPE, TestType::String)
                .field("Tags", TestType::String)
                .field("RequiredTags", TestType::String),
        )
        .record(record(1, TestValue::text("Paints")))
        .record(record(2, TestValue::Raw(u32::MAX.to_le_bytes().to_vec())))
}

fn record(id: u128, first_type: TestValue) -> TestRecord {
    TestRecord::new(
        id,
        ENTITY,
        format!("{ENTITY}.Paint{id}"),
        format!("libs/foundry/records/entities/scitem/fixture/Paint{id}.xml"),
    )
    .value(
        "Components",
        TestValue::Array(vec![
            attachment(first_type, "Paint_First Child", "Required_First"),
            attachment(TestValue::text("Paints"), "Paint_Second", "Required_Second"),
        ]),
    )
}

fn attachment(item_type: TestValue, tags: &str, required_tags: &str) -> TestValue {
    let fields = TestInstance::new(ATTACHMENT)
        .value(TYPE, item_type)
        .value("Tags", TestValue::text(tags))
        .value("RequiredTags", TestValue::text(required_tags));
    TestValue::Struct(TestInstance::new(COMPONENT).value(DEFINITION, TestValue::Struct(fields)))
}
