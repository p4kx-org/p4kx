// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::contract_recipe::CostLine;
use crate::cost_resources;
use crate::guid_text::CatalogGuid;

fn line(resource: Option<&str>, options: Vec<CostLine<'static>>) -> CostLine<'static> {
    CostLine {
        kind: "select",
        count: None,
        slot: None,
        slot_key: None,
        options,
        resource_guid: resource.and_then(|text| text.parse().ok()).map(CatalogGuid),
        item_guid: None,
        quantity_scu: None,
        quantity_units: None,
        min_quality: None,
        quantity_multiplier: None,
        composition_inclusion: None,
        modifiers: Vec::new(),
    }
}

#[test]
fn the_resources_of_a_line_include_those_inside_selections() {
    let first = "a1000000-0000-4000-8000-000000000001";
    let second = "b2000000-0000-4000-8000-000000000002";
    let nested = line(
        None,
        vec![
            line(Some(first), Vec::new()),
            line(None, vec![line(Some(second), Vec::new())]),
        ],
    );
    let mut found = Vec::new();
    cost_resources::collect(&nested, &mut found);
    let expected: Vec<Guid> = [first, second]
        .iter()
        .filter_map(|text| text.parse().ok())
        .collect();
    assert_eq!(found, expected);
}
