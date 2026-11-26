// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::manufacturer_definitions;
use super::number::Number;
use super::record_identity::RecordIdentity;
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::CatalogError;
use crate::catalog_queries::equipment::{
    AuthoredManufacturer, ItemAttachmentPresentation, ItemDefinition, ItemDefinitionComponent,
    ItemDefinitionComponentKind, ItemDefinitionSelection, ItemLocalization, ItemPhysics,
};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{IndexedCatalogRows, StagedCatalogTable};
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::definition_reference::DefinitionStatus;

pub(super) const PATH: &str = "definitions.items";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
    manufacturers: &RecordTargets,
) -> Result<StagedCatalogTable, ReducedError> {
    let mut staged = IndexedCatalogRows::new(scratch, cancel);
    let mut previous = None;
    let selected = ItemDefinitionSelection::Records(CatalogRecordSelection::All);
    let summary = parsed.visit_item_definitions(selected, cancel, |index, snapshot| {
        let [definition] = snapshot.rows() else {
            return Err(ReducedError::IncompleteScope(PATH));
        };
        let header = definition.source();
        let record_index = completeness::cell(PATH, header.record_index())?.value;
        if record_index != Some(&index) || previous.is_some_and(|previous| previous >= index) {
            return Err(ReducedError::IncompleteScope(PATH));
        }
        previous = Some(index);
        let guid = completeness::cell(PATH, &header.guid)?
            .value
            .ok_or(ReducedError::IncompleteScope(PATH))?;
        let mut references = TextRefs::default();
        staged.append(
            (guid.text(), index),
            &Definition::read(definition, index, manufacturers, &mut references)?,
        )?;
        texts.add(
            references,
            snapshot.public_names(),
            snapshot.game_texts(),
            cancel,
        )
    })?;
    completeness::all_fields(PATH, summary.availability(), summary.coverage())?;
    completeness::rows(PATH, summary.owner_count(), summary.row_count())?;
    let table = staged.finish_with_availability(summary.availability())?;
    completeness::rows(PATH, summary.row_count(), table.len())?;
    Ok(table)
}

#[derive(Serialize)]
struct Definition<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    components: Cell<Vec<Component<'read>>>,
}

impl<'read> Definition<'read> {
    fn read(
        source: &'read ItemDefinition<'_>,
        index: u32,
        manufacturers: &RecordTargets,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let header = source.source();
        completeness::value(PATH, header.class_name())?;
        completeness::value(PATH, header.record_path())?;
        let components = completeness::cell(PATH, source.components())?.try_map(|items| {
            let mut components = Vec::new();
            components
                .try_reserve_exact(items.len())
                .map_err(CatalogError::SourceUseAllocation)?;
            for source in items {
                components.push(Component::read(source, index, manufacturers, references)?);
            }
            Ok::<_, ReducedError>(components)
        })?;
        Ok(Self {
            identity: header.into(),
            components,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Component<'read> {
    component_ordinal: Cell<&'read u32>,
    #[serde(flatten)]
    body: Body<'read>,
}

impl<'read> Component<'read> {
    fn read(
        source: &'read ItemDefinitionComponent<'_>,
        index: u32,
        manufacturers: &RecordTargets,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let body = match source.body() {
            ItemDefinitionComponentKind::Attachment(value) => {
                Body::Attachment(completeness::cell(PATH, value)?.try_map(|attachment| {
                    Attachment::read(attachment, index, manufacturers, references).map(Box::new)
                })?)
            }
            ItemDefinitionComponentKind::Physics(value) => {
                Body::Physics(completeness::cell(PATH, value)?.try_map(Physics::read)?)
            }
            ItemDefinitionComponentKind::Purchasable(value) => {
                let text = Text::read(value.display_name(), value.display_type(), references)?;
                Body::Purchasable(Purchase {
                    display_name: text.name,
                    display_type: text.description,
                })
            }
        };
        Ok(Self {
            component_ordinal: completeness::cell(PATH, source.component_ordinal())?,
            body,
        })
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
enum Body<'read> {
    Attachment(Cell<Box<Attachment<'read>>>),
    Physics(Cell<Physics>),
    Purchasable(Purchase<'read>),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Purchase<'read> {
    display_name: Cell<&'read PublicNameRef>,
    display_type: Cell<&'read GameTextRef>,
}

#[derive(Serialize)]
struct Physics {
    mass: Cell<Number<'static>>,
}

impl Physics {
    fn read(source: &ItemPhysics) -> Result<Self, ReducedError> {
        Ok(Self {
            mass: completeness::cell(PATH, source.mass())?.map(Number::from),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Attachment<'read> {
    item_type: Cell<&'read str>,
    item_subtype: Cell<&'read str>,
    size: Cell<Number<'read>>,
    grade: Cell<Number<'read>>,
    tags: Cell<&'read str>,
    required_tags: Cell<&'read str>,
    manufacturer: Manufacturer<'read>,
    localization: Cell<Text<'read>>,
}

impl<'read> Attachment<'read> {
    fn read(
        original: &'read ItemAttachmentPresentation<'_>,
        index: u32,
        manufacturers: &RecordTargets,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            item_type: label(original.item_type())?,
            item_subtype: label(original.item_subtype())?,
            size: completeness::cell(PATH, original.size())?.map(Number::from),
            grade: completeness::cell(PATH, original.grade())?.map(Number::from),
            tags: label(original.tags())?,
            required_tags: label(original.required_tags())?,
            manufacturer: Manufacturer::read(original.manufacturer(), index, manufacturers)?,
            localization: completeness::cell(PATH, original.localization())?
                .try_map(|value| Text::localization(value, references))?,
        })
    }
}

fn label<'read>(value: &'read SourcedValue<&str>) -> Result<Cell<&'read str>, ReducedError> {
    Ok(completeness::cell(PATH, value)?.map(|label| *label))
}

#[derive(Serialize)]
struct Text<'read> {
    name: Cell<&'read PublicNameRef>,
    description: Cell<&'read GameTextRef>,
}

impl<'read> Text<'read> {
    fn localization(
        source: &'read ItemLocalization<'_>,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        Self::read(source.name(), source.description(), references)
    }

    fn read(
        name: &'read SourcedValue<PublicNameRef>,
        description: &'read SourcedValue<GameTextRef>,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let name = completeness::cell(PATH, name)?;
        let description = completeness::cell(PATH, description)?;
        references.name(name.value);
        references.game_text(description.value);
        Ok(Self { name, description })
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
enum Manufacturer<'read> {
    Record(Cell<RecordLink<'read>>),
    Authored(Cell<()>),
}

impl<'read> Manufacturer<'read> {
    fn read(
        original: &'read AuthoredManufacturer<'_>,
        item: u32,
        manufacturers: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        Ok(match original {
            AuthoredManufacturer::Record(cell) => {
                Self::Record(completeness::cell(PATH, cell)?.try_map(|reference| {
                    if matches!(reference.status(), DefinitionStatus::Present) {
                        let index = reference
                            .resolved_index()
                            .ok_or(ReducedError::IncompleteScope(PATH))?;
                        let guid = reference
                            .guid
                            .value()
                            .ok_or(ReducedError::IncompleteScope(PATH))?;
                        manufacturers.verify(manufacturer_definitions::PATH, index, guid.get())?;
                    }
                    RecordLink::from_source(PATH, reference)
                })?)
            }
            AuthoredManufacturer::Authored(cell) => Self::Authored(
                completeness::cell(PATH, cell)?
                    .try_map(|_| Err(ReducedError::UnsupportedManufacturer(item)))?,
            ),
        })
    }
}
