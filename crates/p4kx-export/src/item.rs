// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::RecordTree;

use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::catalog_item_components::item_definition::ItemDefinition;
use crate::catalog_v3::catalog_item_components::selection::ItemSourceSelection;
use crate::item_class::ClassLine;
use crate::playability_rules;
use crate::record_fields::{component, locale_key, structure, text};
use crate::{CatalogError, Playability};

#[path = "item_class_projection.rs"]
mod classification;
#[path = "item_projection_inputs.rs"]
mod inputs;
#[path = "item_paint_facts.rs"]
mod paint_facts;
#[path = "item_playability_projection.rs"]
mod playability_projection;
#[path = "item_projection.rs"]
mod projection;

use inputs::SelectedItem;
use paint_facts::ItemPaintFacts;
pub(crate) use projection::Item;
use projection::ItemIdentity;

/// Returns the playability of the item Record tree `tree`.
pub(crate) fn playability(tree: &RecordTree<'_>) -> Option<Playability> {
    let definition = structure(component(tree, "SAttachableComponentParams"), "AttachDef");
    let localization = structure(definition, "Localization");
    playability_rules::item(tree, locale_key(localization, "Name"))
}

/// Returns whether the item Record tree `tree` has the `AttachDef.Type` of a paint.
pub(crate) fn is_paint(tree: &RecordTree<'_>) -> bool {
    let definition = structure(component(tree, "SAttachableComponentParams"), "AttachDef");
    text(definition, "Type").is_some_and(crate::paint::is_paint_type)
}

/// Projects classifications from the already read physical Item source family.
pub(crate) fn item<'source>(
    tree: &RecordTree<'source>,
    definition: &ItemDefinition<'source>,
    originals: &OriginalFlowFields<'_, '_, 'source>,
    projection: &CatalogProjection<'_, 'source>,
) -> Result<(Item<'source>, ClassLine), CatalogError> {
    projection::check_source(tree, definition, projection)?;
    let selection = ItemSourceSelection::read(tree, originals)?;
    let inputs = SelectedItem::read(definition, &selection)?;
    let description = inputs.description();
    let class = match description.input {
        Some(input) => classification::read(input, projection)?,
        None => classification::absent(description.state, description.cause, projection)?,
    };
    let class_value = description.add_parents(class.value, &projection.origins().uses)?;
    let playability = playability_projection::read(&inputs, projection)?;
    let paint = ItemPaintFacts::read(inputs.attachment());
    let header = definition.source().clone();
    let identity = ItemIdentity::read(&header)?;
    Ok((
        Item {
            header,
            selection,
            class: class_value,
            playability,
            variant_of: None,
            identity,
            paint,
            variant_key: None,
        },
        class.line,
    ))
}

#[cfg(test)]
#[path = "item_projection_semantic_tests.rs"]
mod semantic_tests;
