// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::catalog_crafting_support::{blueprint, build_from};
use crate::crafting_detail_fixture as fixture;
use std::error::Error;

#[test]
fn recipe_quality_positions_keep_original_tier_and_option_occurrences() -> Result<(), Box<dyn Error>>
{
    let (built, catalog) = build_from(&fixture::position_database(), None)?;
    let wire: Value = serde_json::from_slice(built.json())?;
    let blueprint = blueprint::by_guid(&catalog, fixture::quality_position_guid())?;
    let tiers = blueprint.tiers()?;
    let [first, tier] = tiers.as_slice() else {
        return Err("both original inline tiers".into());
    };
    assert_eq!(first["tierIndex"]["value"], 0, "first original tier");
    assert_eq!(tier["tierIndex"]["value"], 1, "second original tier");
    let optional = blueprint::cost(tier, &tier["optional"][0])?;
    assert_eq!(
        optional["position"]["value"],
        json!({"type":"optional","index":1}),
        "the first admitted optional cost retains an original index past a null"
    );
    let nested = first_option(tier, optional)?;
    assert_eq!(
        nested["position"]["value"],
        json!({"type":"option","parent":optional.get("source"),"index":1}),
        "the nested selection refers to the physical optional parent"
    );
    let leaf = first_option(tier, nested)?;
    assert_eq!(
        leaf["position"]["value"],
        json!({"type":"option","parent":nested.get("source"),"index":1}),
        "the ingredient retains an independent identity past a missing option"
    );
    assert_eq!(
        blueprint.debug_name(optional)?,
        "",
        "an empty display key is adequate"
    );
    assert_eq!(
        blueprint.debug_name(nested)?,
        "",
        "repeated display keys retain identity"
    );
    assert!(
        leaf.pointer("/context/modifiers/0/ranges/0").is_some(),
        "the original positional cost still carries its modifier"
    );
    let position = crate::health_origin_assertions::cell(leaf, "position", "present")?;
    assert_eq!(position["containsEncryptedSource"], true);
    let uses = crate::health_origin_assertions::uses(&wire, position)?;
    assert!(
        uses.len() >= 3,
        "option coordinate retains selected concrete owner and original edge"
    );
    let (_, original_catalog) = build_from(&fixture::database(), None)?;
    let other = blueprint::by_guid(&original_catalog, fixture::TWO_TIERS)?;
    let first = other.tiers()?.first().copied().ok_or("mandatory tier")?;
    assert_eq!(
        blueprint::cost(first, &first["mandatory"])?["position"]["value"],
        json!({"type":"mandatory"}),
        "mandatory route has no array index"
    );
    Ok(())
}

fn first_option<'a>(tier: &'a Value, parent: &Value) -> Result<&'a Value, Box<dyn Error>> {
    let reference = parent
        .pointer("/meaning/options/0")
        .ok_or("first admitted option")?;
    blueprint::cost(tier, reference)
}
