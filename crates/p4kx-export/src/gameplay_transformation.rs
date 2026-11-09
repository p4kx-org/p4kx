// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourcedValue, StructValue};

use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::resource_value_read;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

use super::read as source_read;
use super::transformation_contract::{
    DisplayTransformationNodeRef, DisplayTransformationNodeSource, DisplayTransformationPayload,
    DisplayTransformationSource,
};

struct Arena<'context, 'source> {
    context: &'context CatalogRecordContext<'context>,
    notes: &'context mut Vec<FieldError>,
    nodes: Vec<Option<DisplayTransformationNodeSource<'source>>>,
}

/// Retains original owned occurrences in an owner-local flat preorder tree.
pub(crate) fn read<'source>(
    source: &StructValue<'source>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<DisplayTransformationSource<'source>, CatalogError> {
    let mut arena = Arena {
        context,
        notes,
        nodes: Vec::new(),
    };
    let root = arena.append(source, path)?;
    let completed = arena
        .nodes
        .into_iter()
        .map(|node| node.ok_or(ExportAccountingError::UnfilledTransformationNode))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DisplayTransformationSource {
        root,
        nodes: completed,
    })
}

impl<'source> Arena<'_, 'source> {
    fn append(
        &mut self,
        source: &StructValue<'source>,
        path: &str,
    ) -> Result<DisplayTransformationNodeRef, CatalogError> {
        let position = self.nodes.len();
        let node_index =
            u32::try_from(position).map_err(|_| ExportAccountingError::CounterOverflow)?;
        self.nodes.push(None);
        let node = self.node(source, path)?;
        let slot = self
            .nodes
            .get_mut(position)
            .ok_or(ExportAccountingError::UnfilledTransformationNode)?;
        *slot = Some(node);
        Ok(DisplayTransformationNodeRef { node_index })
    }

    fn node(
        &mut self,
        source: &StructValue<'source>,
        path: &str,
    ) -> Result<DisplayTransformationNodeSource<'source>, CatalogError> {
        let fields = CatalogFields::new(source, path, self.context);
        let source_type = fields.source_type()?;
        let (payload, known): (_, &[&str]) = match source.type_name() {
            "CraftingDisplayTransformation_Scale" => (
                Some(DisplayTransformationPayload::Scale {
                    scale: resource_value_read::single_number(&fields, "scale", self.notes)?,
                }),
                &["scale"],
            ),
            "CraftingDisplayTransformation_Sequence" => (
                Some(DisplayTransformationPayload::Sequence {
                    transformations: self.sequence(&fields)?,
                }),
                &["transformations"],
            ),
            name => (leaf(name), &[]),
        };
        fields.check(known, self.notes);
        let payload = source_read::payload(&fields, path, payload, self.notes)?;
        Ok(DisplayTransformationNodeSource {
            source_type,
            payload,
        })
    }

    fn sequence(
        &mut self,
        fields: &CatalogFields<'_, 'source, '_>,
    ) -> Result<SourcedValue<Vec<SourcedValue<DisplayTransformationNodeRef>>>, CatalogError> {
        self.check_sequence_storage(fields);
        let path = fields.location("transformations");
        fields
            .array("transformations", self.notes)?
            .try_map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        let path = format!("{path}[{index}]");
                        self.context
                            .structure_element(
                                value,
                                fields.source().type_name(),
                                &path,
                                self.notes,
                            )?
                            .try_map(|source| self.append(source, &path))
                    })
                    .collect()
            })
    }

    fn check_sequence_storage(&mut self, fields: &CatalogFields<'_, 'source, '_>) {
        let Some(field) = fields
            .source()
            .fields()
            .iter()
            .find(|field| field.name() == "transformations")
        else {
            return;
        };
        let storage = field.storage();
        if storage.is_array() && storage.data_type() == DataType::StrongPointer {
            return;
        }
        self.notes.push(FieldError::Malformed {
            source_type: fields.source().type_name().to_owned(),
            field: fields.location("transformations"),
            expected: "an array declared as StrongPointer",
        });
    }
}

fn leaf(source_type: &str) -> Option<DisplayTransformationPayload> {
    match source_type {
        "CraftingDisplayTransformation_Base" | "CraftingDisplayTransformation_Base_NonRef" => {
            Some(DisplayTransformationPayload::EmptyBase)
        }
        "CraftingDisplayTransformation_ConvertFactorToNegatedPercentChange" => {
            Some(DisplayTransformationPayload::ConvertFactorToNegatedPercentChange)
        }
        "CraftingDisplayTransformation_ConvertFactorToPercentChange" => {
            Some(DisplayTransformationPayload::ConvertFactorToPercentChange)
        }
        "CraftingDisplayTransformation_ConvertValueToFactorOfBaseValue" => {
            Some(DisplayTransformationPayload::ConvertValueToFactorOfBaseValue)
        }
        _ => None,
    }
}
