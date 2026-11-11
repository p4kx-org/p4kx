// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Field, SourcedValue, StructValue, Value};

use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::resource_value_read;
use crate::source_fields::{FieldCapture, FieldError};
use crate::{CatalogError, ExportAccountingError};

use super::read as source_read;
use super::transformation_contract::{
    DisplayTransformationNodeRef, DisplayTransformationNodeSource, DisplayTransformationPayload,
    DisplayTransformationSource,
};

const TRANSFORMATIONS: &str = "transformations";
const SEQUENCE: &str = "CraftingDisplayTransformation_Sequence";

struct Node<'tree, 'source> {
    source: &'tree StructValue<'source>,
    position: usize,
}

struct Sequence<'tree, 'source> {
    node: Node<'tree, 'source>,
    source_type: SourcedValue<&'source str>,
    array: SourcedValue<&'tree [Value<'source>]>,
    slots: Vec<SourcedValue<DisplayTransformationNodeRef>>,
    path_len: usize,
}

struct Arena<'context, 'source> {
    context: &'context CatalogRecordContext<'context>,
    notes: &'context mut Vec<FieldError>,
    capture: &'context dyn FieldCapture<'source>,
    nodes: Vec<Option<DisplayTransformationNodeSource<'source>>>,
}

/// Retains original owned occurrences in an owner-local flat preorder tree.
pub(crate) fn read<'source>(
    source: &StructValue<'source>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    capture: &dyn FieldCapture<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<DisplayTransformationSource<'source>, CatalogError> {
    Arena {
        context,
        notes,
        capture,
        nodes: Vec::new(),
    }
    .read(source, path)
}

impl<'source> Arena<'_, 'source> {
    fn read(
        mut self,
        source: &StructValue<'source>,
        root_path: &str,
    ) -> Result<DisplayTransformationSource<'source>, CatalogError> {
        let root = self.reserve(source);
        let root_ref = node_ref(root.position)?;
        let mut path = root_path.to_owned();
        let mut stack = Vec::new();
        let mut next = Some(root);
        loop {
            if let Some(node) = next.take()
                && let Some(sequence) = self.start(node, &path)?
            {
                stack.push(sequence);
            }
            let Some(sequence) = stack.last_mut() else {
                break;
            };
            path.truncate(sequence.path_len);
            let ordinal = sequence.slots.len();
            if let Some(value) = sequence
                .array
                .value()
                .copied()
                .and_then(|values| values.get(ordinal))
            {
                let name = format!("{TRANSFORMATIONS}[{ordinal}]");
                next = self.child(sequence, &path, &name, value)?;
                if next.is_some() {
                    if !path.is_empty() {
                        path.push('.');
                    }
                    path.push_str(&name);
                }
            } else if let Some(sequence) = stack.pop() {
                self.finish_sequence(sequence, &path)?;
            }
        }
        let nodes = self
            .nodes
            .into_iter()
            .map(|node| node.ok_or(ExportAccountingError::UnfilledTransformationNode))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DisplayTransformationSource {
            root: root_ref,
            nodes,
        })
    }

    fn reserve<'tree>(&mut self, source: &'tree StructValue<'source>) -> Node<'tree, 'source> {
        let position = self.nodes.len();
        self.nodes.push(None);
        Node { source, position }
    }

    fn start<'tree>(
        &mut self,
        node: Node<'tree, 'source>,
        path: &str,
    ) -> Result<Option<Sequence<'tree, 'source>>, CatalogError> {
        let fields = CatalogFields::observed(node.source, path, self.context, self.capture)?;
        let source_type = fields.source_type()?;
        if node.source.type_name() == SEQUENCE {
            let array = sequence_array(node.source, &fields, self.notes)?;
            return Ok(Some(Sequence {
                node,
                source_type,
                array,
                slots: Vec::new(),
                path_len: path.len(),
            }));
        }
        let (payload, known): (_, &[&str]) = match node.source.type_name() {
            "CraftingDisplayTransformation_Scale" => (
                Some(DisplayTransformationPayload::Scale {
                    scale: resource_value_read::single_number(&fields, "scale", self.notes)?,
                }),
                &["scale"],
            ),
            name => (leaf(name), &[]),
        };
        fields.check_observed(known, self.notes)?;
        let payload = source_read::payload(&fields, payload, self.notes)?;
        self.store(
            node.position,
            DisplayTransformationNodeSource {
                source_type,
                payload,
            },
        )?;
        Ok(None)
    }

    fn child<'tree>(
        &mut self,
        sequence: &mut Sequence<'tree, 'source>,
        path: &str,
        name: &str,
        value: &'tree Value<'source>,
    ) -> Result<Option<Node<'tree, 'source>>, CatalogError> {
        let fields =
            CatalogFields::observed(sequence.node.source, path, self.context, self.capture)?;
        let mut next = None;
        let slot = fields
            .structure_element(name, value, self.notes)?
            .try_map(|source| {
                let child = self.reserve(source);
                let reference = node_ref(child.position)?;
                next = Some(child);
                Ok::<_, CatalogError>(reference)
            })?;
        sequence.slots.push(slot);
        Ok(next)
    }

    fn finish_sequence(
        &mut self,
        sequence: Sequence<'_, 'source>,
        path: &str,
    ) -> Result<(), CatalogError> {
        let fields =
            CatalogFields::observed(sequence.node.source, path, self.context, self.capture)?;
        fields.check_observed(&[TRANSFORMATIONS], self.notes)?;
        let payload = source_read::payload(
            &fields,
            Some(DisplayTransformationPayload::Sequence {
                transformations: sequence.array.map(|_| sequence.slots),
            }),
            self.notes,
        )?;
        self.store(
            sequence.node.position,
            DisplayTransformationNodeSource {
                source_type: sequence.source_type,
                payload,
            },
        )
    }

    fn store(
        &mut self,
        position: usize,
        node: DisplayTransformationNodeSource<'source>,
    ) -> Result<(), CatalogError> {
        let slot = self
            .nodes
            .get_mut(position)
            .ok_or(ExportAccountingError::UnfilledTransformationNode)?;
        *slot = Some(node);
        Ok(())
    }
}

fn node_ref(position: usize) -> Result<DisplayTransformationNodeRef, CatalogError> {
    let node_index = u32::try_from(position).map_err(|_| ExportAccountingError::CounterOverflow)?;
    Ok(DisplayTransformationNodeRef { node_index })
}

fn sequence_array<'tree, 'source>(
    source: &'tree StructValue<'source>,
    fields: &CatalogFields<'_, 'source, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<&'tree [Value<'source>]>, CatalogError> {
    let field = source
        .fields()
        .iter()
        .find(|field| field.name() == TRANSFORMATIONS);
    if let Some(field) = field
        && (!field.storage().is_array() || field.storage().data_type() != DataType::StrongPointer)
    {
        notes.push(fields.invalid(
            TRANSFORMATIONS,
            field.value(),
            "an array declared as StrongPointer",
        )?);
    }
    fields.read_value(
        field.map(Field::value),
        TRANSFORMATIONS,
        "a nullable array",
        notes,
        |value| match value {
            Value::Array(values) => Some(values.as_ref()),
            _ => None,
        },
    )
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
