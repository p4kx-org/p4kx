// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

use super::super::{GATED, database};
use super::{REPEATED, Site, Sources, at_site, has_role, requirements, second_generator};

#[test]
fn each_intro_grant_lookup_keeps_its_own_authored_tag_selector() -> TestResult {
    let bytes = database().record(second_generator()).build();
    let fixture = Fixture::open(&bytes, Input::Encrypted, Vec::new())?;
    let original = Sources::read(&fixture)?;
    let (_, catalog) = fixture.diagnostic()?;

    let repeated =
        catalog_contracts::entry(&catalog, &health_origin_fixture::guid(REPEATED), None)?;
    let [first, second] = requirements(repeated)? else {
        return Err("two separate physical requirements for the same tag".into());
    };
    let [earlier, later] = original.repeated;
    check_lookup(&catalog, first, earlier.tag, later.tag)?;
    check_lookup(&catalog, second, later.tag, earlier.tag)?;

    let gated = catalog_contracts::entry(&catalog, &health_origin_fixture::guid(GATED), None)?;
    let [_, _, ungranted] = requirements(gated)? else {
        return Err("the original contract requirement with an empty grant lookup".into());
    };
    let empty = origin::present(ungranted, "introContracts", &json!([]), Some(true))?;
    check_selector(
        &catalog,
        empty,
        original.ungranted.tag,
        original.handler.tag,
    )?;
    origin::closure(&catalog)
}

fn check_lookup(catalog: &Value, requirement: &Value, chosen: Site, sibling: Site) -> TestResult {
    let grants = origin::cell(requirement, "introContracts", "present")?;
    check_selector(catalog, grants, chosen, sibling)?;
    let [first, second] = origin::rows(grants, "/value")? else {
        return Err("the same two original intro IDs answer both lookups".into());
    };
    for grant in [first, second] {
        check_selector(catalog, grant, chosen, sibling)?;
    }
    Ok(())
}

fn check_selector(catalog: &Value, cell: &Value, chosen: Site, sibling: Site) -> TestResult {
    let uses = origin::uses(catalog, cell)?;
    assert!(
        uses.iter()
            .any(|used| at_site(used, chosen) && has_role(used, "$value", "selector")),
        "the lookup retains its own physical required-tag Selector"
    );
    assert!(
        uses.iter().all(|used| !at_site(used, sibling)),
        "a different authored occurrence of the same tag does not choose this result"
    );
    Ok(())
}
