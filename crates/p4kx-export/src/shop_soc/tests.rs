// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use super::{Anchors, ShopComponent, components};

const ENTITY: &str = "4dce4077-9730-ac77-5074-916b3b2700b7";
const OTHER: &str = "4a000000-0000-4000-8000-0000000000aa";
const CLASS: &str = "2845a83c-9c04-450f-99ce-43b8b7da6bec";

fn anchors() -> Anchors {
    let names = HashMap::from([
        ("SCShop_First".to_owned(), ENTITY.to_owned()),
        ("SCShop_Second".to_owned(), OTHER.to_owned()),
    ]);
    Anchors::new(&[ENTITY.to_owned(), OTHER.to_owned()], names)
}

fn found_in(bytes: &[u8]) -> Vec<ShopComponent> {
    components(bytes, &anchors()).components
}

fn push(bytes: &mut Vec<u8>, parts: &[&str]) {
    for part in parts {
        bytes.extend_from_slice(part.as_bytes());
        bytes.push(0);
    }
}

fn stream(parts: &[&str]) -> Vec<u8> {
    let mut bytes = b"junk\0".to_vec();
    push(&mut bytes, parts);
    bytes
}

#[test]
fn the_inventory_stem_is_lower_case_without_folder_and_extension() {
    for value in [
        "Data/scripts/shopinventories/Inv_Admin_GrimHex.json",
        "Scripts\\ShopInventories\\inv_admin_grimhex.xml",
    ] {
        let bytes = stream(&[
            ENTITY,
            "EntityComponentShop",
            "SShopComponentParams",
            "offlineInventoryJSON",
            value,
            "ShopRackGUIDs",
        ]);
        let found = found_in(&bytes);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].entity, ENTITY);
        assert_eq!(found[0].inventory.as_deref(), Some("inv_admin_grimhex"));
    }
}

#[test]
fn an_empty_json_value_does_not_hide_the_xml_name_behind_it() {
    let bytes = stream(&[
        ENTITY,
        "EntityComponentShop",
        "SShopComponentParams",
        "offlineInventoryJSON",
        "offlineInventoryXMLName",
        "Scripts/ShopInventories/Inv_Late.xml",
    ]);
    assert_eq!(found_in(&bytes)[0].inventory.as_deref(), Some("inv_late"));
}

#[test]
fn a_component_without_a_path_names_no_inventory() {
    let bytes = stream(&[
        ENTITY,
        "EntityComponentShop",
        "SShopComponentParams",
        "offlineInventoryJSON",
        "eventSignalRadius",
        "shopInventoryType",
        "ITEM",
    ]);
    let found = found_in(&bytes);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].inventory, None);
}

#[test]
fn the_object_name_ties_a_component_whose_guid_string_is_not_stored_there() {
    let bytes = stream(&[
        "SCShop_First",
        "-12,592,444",
        "SCShop",
        CLASS,
        "EntityComponentShop",
        "SShopComponentParams",
        "offlineInventoryJSON",
        "scripts/shopinventories/inv_landing.json",
    ]);
    let found = found_in(&bytes);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].entity, ENTITY);
    assert_eq!(found[0].inventory.as_deref(), Some("inv_landing"));
}

#[test]
fn a_later_component_repeats_neither_component_name_nor_property_names() {
    let bytes = stream(&[
        ENTITY,
        "EntityComponentShop",
        "SShopComponentParams",
        "offlineInventoryJSON",
        "Data/inv_first.json",
        "eventSignalRadius",
        "ShopRackGUIDs",
        "SCShop_Second",
        "2.0,-5.4,1.5",
        OTHER,
        "scripts/shopinventories/inv_second.json",
    ]);
    let found = found_in(&bytes);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].inventory.as_deref(), Some("inv_first"));
    assert_eq!(found[1].entity, OTHER);
    assert_eq!(found[1].inventory.as_deref(), Some("inv_second"));
}

#[test]
fn a_path_behind_foreign_strings_is_not_the_components() {
    let bytes = stream(&[
        ENTITY,
        "EntityComponentShop",
        "SShopComponentParams",
        "shopInventoryType",
        "ITEM",
        "ShopRackGUIDs",
        "offlineInventoryJSON",
        "Data/inv_other.json",
    ]);
    let scan = components(&bytes, &anchors());
    assert_eq!(scan.components.len(), 0, "nothing is known of the Entity");
    assert_eq!(scan.rejected, 1);
}

#[test]
fn a_path_whose_entity_is_not_stored_stays_unknown() {
    let bytes = stream(&[
        "PropertiesDataCore",
        "__fullyInstanced",
        "__version",
        "2",
        "__type",
        "offlineInventoryJSON",
        "Data/inv_nobody.json",
    ]);
    assert_eq!(found_in(&bytes).len(), 0);
}

#[test]
fn two_foreign_guids_between_anchor_and_path_break_the_tie() {
    let bytes = stream(&[
        ENTITY,
        CLASS,
        "4a000000-0000-4000-8000-0000000000bb",
        "offlineInventoryJSON",
        "Data/inv_far.json",
    ]);
    assert_eq!(found_in(&bytes).len(), 0);
}

#[test]
fn text_without_a_component_gives_nothing() {
    assert_eq!(found_in(b"CrChF\0nothing").len(), 0);
    assert_eq!(found_in(b"EntityComponentShop").len(), 0);
}

#[test]
fn a_populated_unsupported_inventory_does_not_become_an_empty_component() {
    let bytes = stream(&[
        ENTITY,
        "EntityComponentShop",
        "offlineInventoryJSON",
        "Scripts/ShopInventories/unsupported.csv",
    ]);
    let scan = components(&bytes, &anchors());
    assert!(
        scan.malformed,
        "the authored file value could not be represented"
    );
    assert_eq!(
        scan.components,
        Vec::<ShopComponent>::new(),
        "an invalid value is not an authored empty inventory"
    );
    assert_eq!(
        scan.rejected, 0,
        "unattributed valid paths retain their established count"
    );
}

#[test]
fn invalid_utf8_in_an_inventory_value_retains_other_valid_components() {
    let mut bytes = stream(&[ENTITY, "EntityComponentShop", "offlineInventoryJSON"]);
    bytes.extend_from_slice(b"\xff\0");
    push(
        &mut bytes,
        &[
            OTHER,
            "EntityComponentShop",
            "offlineInventoryJSON",
            "Scripts/ShopInventories/valid.json",
        ],
    );
    let scan = components(&bytes, &anchors());
    assert!(
        scan.malformed,
        "populated undecodable value remains visible"
    );
    assert_eq!(
        scan.components.len(),
        1,
        "a separate valid component survives"
    );
    assert_eq!(
        scan.components
            .first()
            .map(|component| component.entity.as_str()),
        Some(OTHER),
        "only the intact Entity remains"
    );
}

#[test]
fn two_positions_between_name_and_path_are_two_objects() {
    let bytes = stream(&[
        "offlineInventoryJSON",
        "SCShop_First",
        "1,2,3",
        "4,5,6",
        "Scripts/ShopInventories/Inv_Of_Second.json",
    ]);
    let scan = components(&bytes, &anchors());
    assert_eq!(scan.components.len(), 0);
    assert_eq!(scan.rejected, 1);
}

#[test]
fn a_guid_stands_in_only_directly_after_the_class_marker() {
    let bytes = stream(&[
        "SCShop_First",
        "1,2,3",
        CLASS,
        "SCShop",
        "offlineInventoryJSON",
        "Data/inv_x.json",
    ]);
    assert_eq!(found_in(&bytes).len(), 0);
}

#[test]
fn a_rejected_path_behind_a_component_name_leaves_the_entity_unknown() {
    let bytes = stream(&[
        ENTITY,
        "EntityComponentShop",
        "something",
        "offlineInventoryJSON",
        "Data/inv_x.json",
    ]);
    let scan = components(&bytes, &anchors());
    assert_eq!(scan.components.len(), 0, "not a component without a file");
    assert_eq!(scan.rejected, 1);
}

#[test]
fn bare_numbers_and_signs_are_no_positions() {
    for text in ["2", "-", ",", "e", "1e5"] {
        let bytes = stream(&[
            "SCShop_First",
            text,
            "offlineInventoryJSON",
            "Data/inv_x.json",
        ]);
        assert_eq!(found_in(&bytes).len(), 0, "{text}");
    }
}
