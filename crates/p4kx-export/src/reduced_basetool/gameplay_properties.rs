// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::number::Number;
use super::record_identity::RecordIdentity;
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::catalog_queries::{
    CatalogRecordSelection, DisplayTransformationNodeRef, DisplayTransformationNodeSource,
    DisplayTransformationPayload, DisplayTransformationSource, GameplayProperty,
    ParsedCatalogQueries,
};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_text_contract::PublicNameRef;
use crate::resource_number::ResourceNumber;

pub(super) const PATH: &str = "definitions.gameplayProperties";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<StagedCatalogTable, ReducedError> {
    let snapshot = parsed.read_gameplay_properties(CatalogRecordSelection::All, cancel)?;
    completeness::inventory(PATH, snapshot.availability, &snapshot.coverage)?;
    let mut rows = StagedCatalogRows::new(scratch, cancel);
    let mut references = TextRefs::default();
    for property in &snapshot.rows {
        rows.append(&GameplayPropertyRow::read(property, &mut references)?)?;
    }
    texts.add_owned_names(references, &snapshot.public_names, cancel)?;
    let table = rows.finish(snapshot.availability)?;
    completeness::rows(PATH, snapshot.rows.len(), table.len())?;
    Ok(table)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameplayPropertyRow<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    property_name: Cell<&'read PublicNameRef>,
    unit_format: Cell<&'read PublicNameRef>,
    display_transformation: Cell<Transformation<'read>>,
}

impl<'read> GameplayPropertyRow<'read> {
    fn read(
        original: &'read GameplayProperty,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let header = &original.source;
        completeness::value(PATH, &header.guid)?;
        completeness::value(PATH, &header.record_index)?;
        completeness::value(PATH, &header.class_name)?;
        completeness::value(PATH, &header.record_path)?;
        let fields = &original.fields;
        let property_name = completeness::cell(PATH, &fields.property_name)?;
        let unit_format = completeness::cell(PATH, &fields.unit_format)?;
        references.name(property_name.value);
        references.name(unit_format.value);
        Ok(Self {
            identity: header.into(),
            property_name,
            unit_format,
            display_transformation: completeness::cell(PATH, &fields.display_transformation)?
                .try_map(Transformation::read)?,
        })
    }
}

#[derive(Serialize)]
struct Transformation<'read> {
    root: &'read DisplayTransformationNodeRef,
    nodes: Vec<Node<'read>>,
}

impl<'read> Transformation<'read> {
    fn read(original: &'read DisplayTransformationSource<'_>) -> Result<Self, ReducedError> {
        let nodes = original
            .nodes
            .iter()
            .map(Node::read)
            .collect::<Result<_, _>>()?;
        Ok(Self {
            root: &original.root,
            nodes,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Node<'read> {
    source_type: Cell<&'read str>,
    payload: Cell<Payload<'read>>,
}

impl<'read> Node<'read> {
    fn read(original: &'read DisplayTransformationNodeSource<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            source_type: completeness::cell(PATH, &original.source_type)?.map(AsRef::as_ref),
            payload: completeness::cell(PATH, &original.payload)?.try_map(Payload::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Payload<'read> {
    EmptyBase,
    ConvertFactorToNegatedPercentChange,
    ConvertFactorToPercentChange,
    ConvertValueToFactorOfBaseValue,
    Scale {
        scale: Cell<Number<'read>>,
    },
    Sequence {
        transformations: Cell<Vec<Cell<&'read DisplayTransformationNodeRef>>>,
    },
}

impl<'read> Payload<'read> {
    fn read(original: &'read DisplayTransformationPayload) -> Result<Self, ReducedError> {
        Ok(match original {
            DisplayTransformationPayload::EmptyBase => Self::EmptyBase,
            DisplayTransformationPayload::ConvertFactorToNegatedPercentChange => {
                Self::ConvertFactorToNegatedPercentChange
            }
            DisplayTransformationPayload::ConvertFactorToPercentChange => {
                Self::ConvertFactorToPercentChange
            }
            DisplayTransformationPayload::ConvertValueToFactorOfBaseValue => {
                Self::ConvertValueToFactorOfBaseValue
            }
            DisplayTransformationPayload::Scale { scale } => Self::Scale {
                scale: completeness::cell(PATH, scale)?.try_map(|original| {
                    let ResourceNumber::Single(_) = original else {
                        return Err(ReducedError::IncompleteScope(PATH));
                    };
                    Ok(Number::from(original))
                })?,
            },
            DisplayTransformationPayload::Sequence { transformations } => Self::Sequence {
                transformations: completeness::list(PATH, transformations, Ok)?,
            },
        })
    }
}
