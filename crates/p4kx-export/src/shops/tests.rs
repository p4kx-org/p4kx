// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::Guid;

use super::{RegistryShop, ShopSummary, list};
use crate::contract::StarmapObject;
use crate::guid_text::CatalogGuid;
use crate::shop_containers::Containers;
use crate::shop_links::Links;
use crate::shop_registry::RegistryId;

const ROOT: &str = "4a269932-183e-362d-aacc-188e7debbea9";
const BODY: &str = "4ce1f681-b479-5269-0536-3d7f02fdccaf";

fn object(id: u8, class_name: &'static str, entity_name: Option<&str>) -> StarmapObject<'static> {
    StarmapObject {
        guid: CatalogGuid(Guid::from_stored([id; 16])),
        class_name,
        record_path: "",
        name: crate::contract::LocalisedText::default(),
        r#type: None,
        type_guid: None,
        parent_guid: None,
        hide_in_starmap: None,
        system: None,
        entity_name: entity_name.map(str::to_owned),
        placed_under_guid: None,
        position: None,
        mining_claim: false,
    }
}

fn entry(location: &str, entity_id: &str) -> RegistryShop {
    RegistryShop {
        platform: "StantonSystem".to_owned(),
        id: RegistryId::Fixture(format!(
            "{ROOT},{BODY},{entity_id},{}",
            entity_id.to_ascii_uppercase()
        )),
        super_name: format!("Root,Body,{location},SCShop_X"),
        default_value: String::new(),
        shop_name: String::new(),
    }
}

#[test]
fn a_name_of_two_starmap_objects_links_to_none_and_is_counted() {
    let starmap = [
        object(1, "Spot", None),
        object(2, "Other", Some("spot")),
        object(3, "Unique", None),
    ];
    let links = Links::new(&starmap);
    let id = |n: u8| format!("4a000000-0000-4000-8000-0000000000{n:02x}");
    let registry = [
        entry("Spot", &id(1)),
        entry("Unique", &id(2)),
        entry("Nowhere", &id(3)),
    ];
    let mut summary = ShopSummary::default();
    let lists = HashMap::new();
    let shops = list(
        &registry,
        &Containers::default(),
        &links,
        &lists,
        &mut summary,
        |_, _| Ok(()),
    )
    .expect("the synthetic listing callback succeeds")
    .shops;
    assert_eq!(shops.len(), 3);
    assert_eq!((summary.linked, summary.unlinked), (1, 2));
    assert_eq!(summary.ambiguous_link, 1, "only Spot is ambiguous");
    let spot = shops
        .iter()
        .find(|shop| shop.location.as_deref() == Some("Spot"));
    assert!(spot.is_some_and(|shop| shop.starmap_object_guid.is_none()));
}

#[test]
fn a_key_is_lower_case_and_a_non_guid_element_is_malformed() {
    let links = Links::new(&[]);
    let upper = entry("Place", "4A000000-0000-4000-8000-0000000000AA");
    let lower = entry("Place", "4a000000-0000-4000-8000-0000000000aa");
    let broken = entry("Place", "not-a-guid");
    let registry = [upper, lower, broken];
    let mut summary = ShopSummary::default();
    let lists = HashMap::new();
    let shops = list(
        &registry,
        &Containers::default(),
        &links,
        &lists,
        &mut summary,
        |_, _| Ok(()),
    )
    .expect("the synthetic listing callback succeeds")
    .shops;
    assert_eq!(shops.len(), 1, "the case-duplicate is dropped");
    assert_eq!((summary.duplicate, summary.malformed), (1, 1));
    let shop = &shops[0];
    assert_eq!(shop.system, "stanton");
    assert!(
        shop.key.ends_with(&shop.entity_guid),
        "entityGuid is the last element"
    );
    assert_eq!(shop.key, shop.key.to_ascii_lowercase());
}
