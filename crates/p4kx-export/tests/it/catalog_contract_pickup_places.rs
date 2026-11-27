// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::{A, B, C, SUB_ONE, SUB_TWO, TestResult, build, entry, v3};
use crate::commodity_v3_fixture::text_row;

pub(crate) fn places<'a>(
    catalog: &'a Value,
    places: &'a Value,
) -> TestResult<Vec<(&'a str, &'a str)>> {
    places
        .as_array()
        .ok_or("places array")?
        .iter()
        .map(|place| {
            Ok((
                place["objectType"].as_str().unwrap_or_default(),
                place_name(catalog, place)?.unwrap_or_default(),
            ))
        })
        .collect()
}

fn place_name<'a>(catalog: &'a Value, row: &Value) -> TestResult<Option<&'a str>> {
    let name = row.get("place").ok_or("place name cell")?;
    if name.is_null() {
        return Ok(None);
    }
    let Some(id) = v3::sourced(name)? else {
        return Ok(None);
    };
    let chosen = text_row(catalog, "gameTexts", id)?;
    let display = chosen
        .pointer("/translations/value/en/displayText")
        .ok_or("English place display text cell")?;
    Ok(v3::sourced(display)?.and_then(Value::as_str))
}

#[test]
fn the_pickup_places_put_the_system_and_planets_first_and_keep_every_non_star() -> TestResult {
    let (_, catalog) = build(None)?;
    let pickup = |guid, sub| places(&catalog, &entry(&catalog, guid, sub)?["pickupPlaces"]);
    assert_eq!(
        pickup(A, None)?,
        [
            ("SolarSystem", "Stanton"),
            ("Planet", "Hurston"),
            ("Station", "Everus"),
        ],
        "the star is skipped, the system comes first, then the planet, then the rest"
    );
    assert_eq!(
        pickup(A, Some(SUB_ONE))?,
        [("Planet", "Hurston")],
        "a sub contract's own location replaces the contract's"
    );
    assert_eq!(
        entry(&catalog, A, Some(SUB_TWO))?["pickupPlaces"],
        entry(&catalog, A, None)?["pickupPlaces"],
        "a sub contract without places takes the contract's"
    );
    assert_eq!(pickup(B, None)?.len(), 7, "every eligible original place");
    assert_eq!(pickup(C, None)?, [], "none named");
    Ok(())
}
