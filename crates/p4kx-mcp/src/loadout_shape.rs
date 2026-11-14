// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_assembly::{
    Attachment, EntityNode, PortDefinition, PortNode, PortTransform, PortVisibility,
};

use crate::budget::Budget;
use crate::budget_text;
use crate::error::ToolError;
use crate::loadout_error;
use crate::loadout_shapes::{
    Counts, DefinitionShape, EntityShape, LoadoutResult, PortShape, TypeShape,
};

/// The most issues listed per Entity.
const MAX_ISSUES: usize = 64;
/// The most item types listed per port.
const MAX_TYPES: usize = 16;
/// The most sub-types listed per item type.
const MAX_SUB_TYPES: usize = 16;

pub(crate) fn envelope(
    archive: &Path,
    node: &EntityNode,
    counts: Counts,
    total: usize,
    deeper: usize,
) -> Result<LoadoutResult, ToolError> {
    let mut text = Budget::new();
    Ok(LoadoutResult {
        archive: budget_text::display(&mut text, &archive.display())?,
        entity: entity(node, &mut text)?,
        counts,
        total,
        deeper,
        ports: Vec::new(),
        next_cursor: None,
    })
}

fn entity(node: &EntityNode, text: &mut Budget) -> Result<EntityShape, ToolError> {
    Ok(EntityShape {
        guid: budget_text::display(text, &node.guid())?,
        record_path: text.text(node.record_path())?,
        name: text.text(node.name())?,
        class_name: text.text(node.class_name())?,
        docked_vehicle: node.is_docked_vehicle(),
        expanded: node.is_expanded(),
        issues: node
            .issues()
            .iter()
            .take(MAX_ISSUES)
            .map(|error| loadout_error::shape(error, None, text))
            .collect::<Result<_, _>>()?,
        issue_count: node.issues().len(),
        notes: node
            .notes()
            .iter()
            .map(|error| loadout_error::shape(error, None, text))
            .collect::<Result<_, _>>()?,
    })
}

pub(crate) fn port(path: &str, depth: usize, port: &PortNode) -> Result<PortShape, ToolError> {
    let mut text = Budget::new();
    let (entity, unresolved) = match port.attachment() {
        Attachment::Empty => (None, None),
        Attachment::Entity(node) => (Some(entity(node, &mut text)?), None),
        Attachment::Unresolved(failed) => (
            None,
            Some(loadout_error::shape(
                failed.error(),
                Some(failed.target()),
                &mut text,
            )?),
        ),
    };
    Ok(PortShape {
        path: text.text(path)?,
        depth,
        name: text.text(port.name())?,
        source: port.source().code(),
        visibility: match port.visibility() {
            PortVisibility::Unspecified => None,
            stated => Some(stated.code()),
        },
        definition: port
            .definition()
            .map(|value| definition(value, &mut text))
            .transpose()?,
        entity,
        unresolved,
    })
}

fn definition(
    definition: &PortDefinition,
    text: &mut Budget,
) -> Result<DefinitionShape, ToolError> {
    let helper = definition
        .transform()
        .map(PortTransform::helper)
        .filter(|helper| !helper.is_empty())
        .map(|helper| text.text(helper))
        .transpose()?;
    Ok(DefinitionShape {
        display_name: text.text(definition.display_name())?,
        flags: text.text(definition.flags())?,
        min_size: definition.min_size(),
        max_size: definition.max_size(),
        types: types(definition, text)?,
        type_count: definition.types().len(),
        helper,
    })
}

fn types(definition: &PortDefinition, text: &mut Budget) -> Result<Vec<TypeShape>, ToolError> {
    definition
        .types()
        .iter()
        .take(MAX_TYPES)
        .map(|item| {
            Ok(TypeShape {
                name: text.text(item.name())?,
                sub_types: item
                    .sub_types()
                    .iter()
                    .take(MAX_SUB_TYPES)
                    .map(|value| text.text(value))
                    .collect::<Result<_, _>>()?,
                sub_type_count: item.sub_types().len(),
            })
        })
        .collect()
}
