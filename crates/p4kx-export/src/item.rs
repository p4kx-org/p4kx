// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::RecordTree;

use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::catalog_item_components::item_attachment;
use crate::catalog_v3::catalog_item_components::item_definition::ItemDefinition;
use crate::catalog_v3::catalog_item_components::selection::ItemSourceSelection;
use crate::item_class::ClassLine;
use crate::playability_rules;
use crate::record_fields::{locale_key, structure};
use crate::variant_decision::VariantDecision;
use crate::{CatalogError, Playability};

#[path = "item_class_projection.rs"]
mod classification;
#[path = "item_projection_inputs.rs"]
mod inputs;
#[path = "item_paint_facts.rs"]
mod paint_facts;
#[path = "paint_compatibility_input.rs"]
pub(crate) mod paint_input;
pub(crate) use paint_input::PaintInput;
#[path = "item_playability_projection.rs"]
pub(crate) mod playability_projection;
#[path = "item_projection.rs"]
mod projection;

use inputs::SelectedItem;
use paint_facts::ItemPaintFacts;
pub use projection::Item;
use projection::ItemIdentity;
pub use projection::ItemSource;

/// Returns the playability of the item Record tree `tree`.
pub(crate) fn playability(tree: &RecordTree<'_>) -> Option<Playability> {
    let localization = structure(item_attachment::fields(tree), "Localization");
    playability_rules::item(tree, locale_key(localization, "Name"))
}

/// Returns whether the item Record tree `tree` has the `AttachDef.Type` of a paint.
pub(crate) fn is_paint(tree: &RecordTree<'_>) -> bool {
    item_attachment::raw_type(tree).is_some_and(crate::paint::is_paint_type)
}

/// Projects classifications from the already read physical Item source family.
pub(crate) fn item<'source>(
    tree: &RecordTree<'source>,
    definition: &ItemDefinition<'source>,
    originals: &OriginalFlowFields<'_, '_, 'source>,
    projection: &CatalogProjection<'_, 'source>,
) -> Result<(Item<'source>, ClassLine), CatalogError> {
    let read = project_source(tree, definition, originals, projection, |inputs| {
        inputs.paint()
    })?;
    let identity = ItemIdentity::read(read.source.source())?;
    Ok((
        Item {
            source: read.source,
            variant_of: VariantDecision::unresolved(&projection.origins().uses),
            identity,
            paint: read.extra,
            variant_components: crate::item_variant::component_key(tree),
        },
        read.line,
    ))
}

pub(crate) fn source<'source>(
    tree: &RecordTree<'source>,
    definition: &ItemDefinition<'source>,
    originals: &OriginalFlowFields<'_, '_, 'source>,
    projection: &CatalogProjection<'_, 'source>,
) -> Result<ItemSource<'source>, CatalogError> {
    Ok(project_source(tree, definition, originals, projection, |_| ())?.source)
}

struct ProjectedSource<'source, T> {
    source: ItemSource<'source>,
    line: ClassLine,
    extra: T,
}

fn project_source<'source, T>(
    tree: &RecordTree<'source>,
    definition: &ItemDefinition<'source>,
    originals: &OriginalFlowFields<'_, '_, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    extra: impl FnOnce(&SelectedItem<'_, 'source>) -> T,
) -> Result<ProjectedSource<'source, T>, CatalogError> {
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
    let extra = extra(&inputs);
    let header = definition.source().clone();
    Ok(ProjectedSource {
        source: ItemSource {
            header,
            selection,
            class: class_value,
            playability,
        },
        line: class.line,
        extra,
    })
}

#[cfg(test)]
#[path = "item_projection_semantic_tests.rs"]
mod semantic_tests;
