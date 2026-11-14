// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::RecordTree;

use crate::Playability;
use crate::contract::Item;
use crate::guid_text::CatalogGuid;
use crate::item_class::ClassLine;
use crate::playability_rules;
use crate::record_fields::{
    class_name, component, integer, locale_key, reference, single, structure, text,
};
use crate::texts::Texts;

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

/// Returns the item the Record tree `tree` describes and the `Class:` line of its English
/// description, its texts resolved by `texts`.
pub(crate) fn item<'f>(tree: &RecordTree<'f>, texts: &Texts<'f>) -> (Item<'f>, ClassLine) {
    let definition = structure(component(tree, "SAttachableComponentParams"), "AttachDef");
    let localization = structure(definition, "Localization");
    let description = texts.localised(locale_key(localization, "Description"));
    let line = ClassLine::read(description.en.as_deref());
    let physics = structure(
        component(tree, "SEntityPhysicsControllerParams"),
        "PhysType",
    );
    let name_key = locale_key(localization, "Name");
    let r#type = text(definition, "Type");
    let item = Item {
        source_record_index: tree.record_index(),
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        name: texts.localised(name_key),
        description,
        r#type,
        sub_type: text(definition, "SubType"),
        size: integer(definition, "Size"),
        grade: integer(definition, "Grade"),
        mass: single(physics, "Mass"),
        manufacturer_guid: reference(definition, "Manufacturer"),
        tags: words(text(definition, "Tags")),
        display_type: texts.localised(locale_key(
            component(tree, "SCItemPurchasableParams"),
            "displayType",
        )),
        class: line.class(),
        playability: playability(tree),
        variant_of: None,
        stats: None,
        variant_key: None,
        required_tags: if r#type.is_some_and(crate::paint::is_paint_type) {
            words(text(definition, "RequiredTags"))
        } else {
            Vec::new()
        },
    };
    (item, line)
}

fn words(value: Option<&str>) -> Vec<&str> {
    value.map_or_default(|tags| tags.split_whitespace().collect())
}
