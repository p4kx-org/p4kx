// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use super::text_routes::{TextDomain, TextPresence, check_string, check_text_route};
use super::{Inventory, Result, family_fields, released_schema};

const MANUFACTURER: &str = "manufacturers";
const ROW: &str = "/definitions/manufacturers/rows/[]";

#[test]
fn released_v2_manufacturer_fields_have_explicit_dispositions() -> Result {
    let legacy = released_schema()?;
    let inventory = Inventory::collect(&legacy)?;
    let released = family_fields(&inventory, MANUFACTURER)?;
    let current = p4kx_export::catalog_schema();
    let mut classified = BTreeSet::new();
    for field in ["/className", "/code", "/guid", "/recordPath"] {
        classified.insert(field.to_owned());
        check_string(&current, &format!("{ROW}{field}/value"))?;
    }
    for (field, domain) in [
        ("name", TextDomain::PublicName),
        ("description", TextDomain::GameText),
    ] {
        for suffix in ["", "/key", "/en", "/de"] {
            classified.insert(format!("/{field}{suffix}"));
        }
        check_text_route(
            &current,
            &format!("{ROW}/localization/value"),
            field,
            domain,
            TextPresence::Direct,
        )?;
    }
    assert_eq!(
        released,
        classified.iter().map(String::as_str).collect(),
        "released Manufacturer migration coverage"
    );
    Ok(())
}
