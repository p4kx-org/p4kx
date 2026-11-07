// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, StructValue, is_placeholder_key};

use crate::contract::Commodity;
use crate::guid_text::CatalogGuid;
use crate::record_fields::{
    class_name, component, element, locale_key, reference, single, structure,
};
use crate::resource_group::Groups;
use crate::texts::Texts;

/// Returns the commodity the Record tree `tree` describes, its texts resolved by `texts`.
///
/// A name or description the commodity lacks or marks as a placeholder is taken from the
/// `ResourceType` Record `resource_type` instead. Its type is the group in `groups` that lists
/// that `ResourceType`, its subtype the GUID of the `ResourceType`.
pub(crate) fn commodity<'f>(
    tree: &RecordTree<'f>,
    texts: &Texts<'f>,
    resource_type: Option<&RecordTree<'f>>,
    groups: &Groups,
    subtype: Option<CatalogGuid>,
) -> Commodity<'f> {
    let purchasable = component(tree, "SCItemPurchasableParams");
    let display = element(
        Some(tree.root()),
        "StaticEntityClassData",
        "EntityUIDisplayParams",
    );
    let resource = resource_type.map(RecordTree::root);
    let name = usable(
        locale_key(purchasable, "displayName"),
        locale_key(resource, "displayName"),
    );
    let description = usable(
        locale_key(display, "displayDescription"),
        locale_key(resource, "description"),
    );
    Commodity {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        name: texts.localised(name),
        description: texts.localised(description),
        type_guid: subtype.and_then(|subtype| groups.of_resource(&subtype.0)),
        subtype_guid: subtype,
        display_type: texts.localised(locale_key(purchasable, "displayType")),
        density_grams_per_cubic_centimetre: density(resource),
        refined_subtype_guid: reference(resource, "refinedVersion"),
    }
}

/// Returns the density in grams per cubic centimetre the `densityType` of the `ResourceType`
/// struct `resource` holds.
fn density(resource: Option<&StructValue<'_>>) -> Option<f32> {
    let density = structure(resource, "densityType");
    let unit = structure(density, "densityUnit");
    single(unit, "gramsPerCubicCentimeter")
}

fn usable<'f>(own: Option<&'f str>, fallback: Option<&'f str>) -> Option<&'f str> {
    match own {
        Some(key) if !is_placeholder_key(key) => own,
        _ => fallback.or(own),
    }
}

/// Returns the commodity the `ResourceType` Record tree `tree` stands for, which no commodity
/// Record carries: its GUID, Record and texts are those of the `ResourceType`, its type the group
/// in `groups` that lists it, its subtype its own GUID.
pub(crate) fn resource_commodity<'f>(
    tree: &RecordTree<'f>,
    texts: &Texts<'f>,
    groups: &Groups,
) -> Commodity<'f> {
    let guid = tree.guid();
    Commodity {
        guid: CatalogGuid(guid),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        name: texts.localised(locale_key(Some(tree.root()), "displayName")),
        description: texts.localised(locale_key(Some(tree.root()), "description")),
        type_guid: groups.of_resource(&guid),
        subtype_guid: Some(CatalogGuid(guid)),
        display_type: texts.localised(None),
        density_grams_per_cubic_centimetre: density(Some(tree.root())),
        refined_subtype_guid: reference(Some(tree.root()), "refinedVersion"),
    }
}
