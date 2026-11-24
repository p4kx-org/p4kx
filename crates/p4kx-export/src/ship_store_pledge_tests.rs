// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use serde_json::{Value, json};

use super::{
    DATA, ShipPledgeProductionStatus, ShipStorePledgeScope, date, parse, ship_store_pledges,
};
use crate::ship_store_slugs::StoreSlugs;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn the_reviewed_pledges_keep_their_exact_identity_and_scope() -> TestResult {
    let pledges = ship_store_pledges()?;
    let snapshot = pledges.snapshot();
    assert_eq!(
        snapshot.scope(),
        ShipStorePledgeScope::ReviewedCandidates,
        "review scope"
    );
    assert_eq!(
        snapshot.capture_date(),
        "2026-10-28",
        "manual source capture"
    );
    assert_eq!(
        snapshot.reviewed_date(),
        "2026-11-24",
        "candidate status review"
    );
    for (id, slug, name, manufacturer) in [
        (63, "Javelin", "Javelin", "AEGS"),
        (86, "Hull-D", "Hull D", "MISC"),
        (170, "Kraken", "Kraken", "DRAK"),
        (317, "Odin", "Odin", "ANVL"),
    ] {
        let row = pledges.by_rsi_id(id).ok_or("reviewed Store identity")?;
        assert_eq!(
            (
                row.slug(),
                row.store_name(),
                row.manufacturer_code(),
                row.production_status()
            ),
            (
                slug,
                name,
                manufacturer,
                ShipPledgeProductionStatus::InConcept
            ),
            "the bundled curated facts for RSI id {id}"
        );
    }
    assert_eq!(pledges.rows().len(), 27, "the reviewed bundled cohort");
    assert!(
        pledges.by_rsi_id(224).is_none() && pledges.by_rsi_id(244).is_none(),
        "reviewed contradictory release evidence excludes these candidates"
    );
    Ok(())
}

#[test]
fn unbound_and_retired_mapping_outcomes_stay_separate_from_pledges() -> TestResult {
    let mapping = StoreSlugs::parse(
        r#"{"format":1,"ships":[
        {"rsiId":99001,"slug":"Fixture-Unbound","storeName":"Fixture Unbound","className":null,"guid":null,"retired":false},
        {"rsiId":99002,"slug":"Fixture-Retired","storeName":"Fixture Retired","className":"FixtureRetired","guid":"00000000-0000-4000-8000-000000000002","retired":true}
    ]}"#,
    )?;
    let pledges = ship_store_pledges()?;
    let resolved = mapping.resolve(std::iter::empty(), false);
    let [retired] = resolved.exported.as_slice() else {
        return Err("exactly one retired mapping witness".into());
    };
    assert_eq!(
        resolved.summary.unmapped, 1,
        "the Store map recognizes one unbound entry"
    );
    assert!(retired.is_retired(), "retirement belongs to the Store map");
    assert_eq!(retired.rsi_id(), 99002, "the retired fixture identity");
    assert!(
        retired.ship_guid().is_none(),
        "the retired mapping has no active Ship"
    );
    assert!(
        pledges.by_rsi_id(99001).is_none(),
        "the unbound fixture has no reviewed pledge status"
    );
    assert!(
        pledges.by_rsi_id(99002).is_none(),
        "retirement does not imply a reviewed concept"
    );
    assert!(
        pledges.by_rsi_id(63).is_some(),
        "the reviewed concept comes from the separate source"
    );
    Ok(())
}

#[test]
fn manifest_refuses_unknown_source_invalid_dates_and_nonpositive_revision() -> TestResult {
    let baseline: Value = serde_json::from_str(DATA)?;
    for (path, value) in [
        ("/snapshot/source", json!("wrong-source")),
        ("/snapshot/sourceRevision", json!("bad-digest")),
        ("/snapshot/captureDate", json!("2026-02-29")),
        ("/snapshot/reviewedDate", json!("2026-10-27")),
        ("/snapshot/revision", json!(0)),
        ("/snapshot/scope", json!("currentPopulation")),
        ("/rows/0/productionStatus", json!("flight-ready")),
    ] {
        let mut modified = baseline.clone();
        *modified
            .pointer_mut(path)
            .ok_or("the fixture mutation address")? = value;
        assert!(
            parse(&serde_json::to_string(&modified)?).is_err(),
            "refuse {path}"
        );
    }
    assert_eq!(
        date("2000-02-29"),
        Some((2000, 2, 29)),
        "the supported leap day"
    );
    assert_eq!(
        date("1900-02-29"),
        None,
        "a common century is not a leap year"
    );
    assert_eq!(
        date("2026-1-02"),
        None,
        "dates must have fixed decimal width"
    );
    Ok(())
}

#[test]
fn manifest_refuses_duplicate_ids_duplicate_slugs_and_out_of_order_rows() -> TestResult {
    let baseline: Value = serde_json::from_str(DATA)?;
    for (key, value) in [
        ("rsiId", json!(63)),
        ("slug", json!("Javelin")),
        ("rsiId", json!(1)),
    ] {
        let mut candidate = baseline.clone();
        let rows = candidate
            .pointer_mut("/rows")
            .and_then(Value::as_array_mut)
            .ok_or("pledge rows")?;
        let javelin = rows
            .iter()
            .position(|row| row["rsiId"] == 63)
            .ok_or("the Javelin row")?;
        let later = rows
            .get_mut(javelin + 1)
            .and_then(Value::as_object_mut)
            .ok_or("a later object")?;
        later.insert(key.to_owned(), value);
        assert!(
            parse(&serde_json::to_string(&candidate)?).is_err(),
            "refuse a conflicting {key}"
        );
    }
    Ok(())
}
