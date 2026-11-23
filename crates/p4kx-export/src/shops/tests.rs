// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use p4kx_extraction::{Guid, SourceUses, SourceValue, SourceValueState};

use super::{RegistryShop, ShopSummary, list};
use crate::contract::StarmapObject;
use crate::guid_text::CatalogGuid;
use crate::shop_containers::Containers;
use crate::shop_inventory_resolution::ShopInventoryCells;
use crate::shop_links::{Links, ShopLinkSubject};
use crate::shop_registry::{RegistryAttribute, RegistryAttributeOrigin, RegistryId};

const ROOT: &str = "4a269932-183e-362d-aacc-188e7debbea9";
const BODY: &str = "4ce1f681-b479-5269-0536-3d7f02fdccaf";

fn object(id: u8, class_name: &'static str, entity_name: Option<&str>) -> StarmapObject<'static> {
    StarmapObject {
        guid: CatalogGuid(Guid::from_stored([id; 16])),
        record_index: u32::from(id),
        class_name,
        name: SourceUses::new().value(SourceValue::Absent, &[], true),
        r#type: None,
        system: None,
        entity_name: entity_name.map(str::to_owned),
        placed_under_guid: None,
        position: None,
        placement_proof: None,
        mining_claim: false,
        location_origin: None,
        shop_link_header: None,
    }
}

fn entry(location: &str, entity_id: &str) -> RegistryShop {
    RegistryShop {
        platform: Arc::new(RegistryAttribute::fixture("StantonSystem".to_owned())),
        id: RegistryId::Fixture(format!(
            "{ROOT},{BODY},{entity_id},{}",
            entity_id.to_ascii_uppercase()
        )),
        id_origin: RegistryAttributeOrigin::Fixture,
        super_name: RegistryAttribute::fixture(format!("Root,Body,{location},SCShop_X")),
        default_value: RegistryAttribute::fixture(String::new()),
        shop_name: RegistryAttribute::fixture(String::new()),
    }
}

#[test]
fn a_name_of_two_starmap_objects_links_to_none_and_is_counted() {
    let starmap = [
        object(1, "Spot", None),
        object(2, "Other", Some("spot")),
        object(3, "Unique", None),
    ];
    let links = Links::new(starmap.iter().map(ShopLinkSubject::from), true);
    let id = |n: u8| format!("4a000000-0000-4000-8000-0000000000{n:02x}");
    let registry = [
        entry("Spot", &id(1)),
        entry("Unique", &id(2)),
        entry("Nowhere", &id(3)),
    ];
    let mut summary = ShopSummary::default();
    let lists = HashMap::new();
    let sources = SourceUses::new();
    let shops = list(
        &registry,
        &Containers::default(),
        &links,
        &lists,
        &mut summary,
        &sources,
        |_, _| Ok(ShopInventoryCells::unscanned(&sources)),
    )
    .expect("the synthetic listing callback succeeds")
    .shops;
    assert_eq!(shops.len(), 3);
    assert_eq!((summary.linked, summary.unlinked), (1, 2));
    assert_eq!(summary.ambiguous_link, 1, "only Spot is ambiguous");
    let spot = shops
        .iter()
        .find(|shop| shop.registry.location.value().map(String::as_str) == Some("Spot"));
    assert!(spot.is_some_and(|shop| shop.starmap_object_guid.state() == SourceValueState::Invalid));
}

#[test]
fn a_key_is_lower_case_and_a_non_guid_element_is_malformed() {
    let links = Links::new([], true);
    let upper = entry("Place", "4A000000-0000-4000-8000-0000000000AA");
    let lower = entry("Place", "4a000000-0000-4000-8000-0000000000aa");
    let broken = entry("Place", "not-a-guid");
    let registry = [upper, lower, broken];
    let mut summary = ShopSummary::default();
    let lists = HashMap::new();
    let sources = SourceUses::new();
    let shops = list(
        &registry,
        &Containers::default(),
        &links,
        &lists,
        &mut summary,
        &sources,
        |_, _| Ok(ShopInventoryCells::unscanned(&sources)),
    )
    .expect("the synthetic listing callback succeeds")
    .shops;
    assert_eq!(shops.len(), 1, "the case-duplicate is dropped");
    assert_eq!((summary.duplicate, summary.malformed), (1, 1));
    let shop = &shops[0];
    assert_eq!(
        shop.registry.system.value().map(String::as_str),
        Some("stanton"),
        "platform conversion"
    );
    let key = shop.registry.key.value().expect("admitted normalized key");
    let entity = shop
        .registry
        .entity_guid
        .value()
        .expect("admitted entity GUID");
    assert!(key.ends_with(entity), "entityGuid is the last element");
    assert_eq!(key, &key.to_ascii_lowercase(), "normalized placement key");
}

#[test]
fn incomplete_name_survey_does_not_certify_a_matching_class()
-> Result<(), Box<dyn std::error::Error>> {
    let sources = SourceUses::new();
    let location = sources.value(SourceValue::Present("Spot".to_owned()), &[], true);
    let mut class = object(1, "Spot", None);
    class.shop_link_header = Some(crate::contract_starmap::ShopLinkHeader {
        guid: sources.origin(&[], true),
        class_name: sources.origin(&[], true),
    });
    let certain = Links::new([ShopLinkSubject::from(&class)], true);
    assert!(
        certain
            .resolve(&location, &sources)?
            .origin()
            .source_set_complete(),
        "the class input alone is sufficient only after a complete survey"
    );
    let placed = object(2, "Elsewhere", Some("spot"));
    let known = Links::new([ShopLinkSubject::from(&class)], false);
    let location_link = known.resolve(&location, &sources)?;
    assert_eq!(
        location_link.value(),
        Some(&class.guid),
        "a known match stays available"
    );
    assert!(
        !location_link.origin().source_set_complete(),
        "unseen placements can change the match"
    );

    let complete = Links::new([&class, &placed].map(ShopLinkSubject::from), true);
    let location_link = complete.resolve(&location, &sources)?;
    assert_eq!(
        location_link.state(),
        SourceValueState::Invalid,
        "a differently named class with a matching placed name competes"
    );
    Ok(())
}
