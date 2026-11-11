// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The description of contracts in the Basetool catalog.

use crate::catalog_contracts::{A, C, E, TestResult, build, entry, v3};

#[test]
fn the_description_is_resolved_like_the_title_and_falls_back_to_the_broker_entry() -> TestResult {
    let (_, catalog) = build(None)?;
    let projections = v3::Projections::read(&catalog)?;
    assert_eq!(
        projections.text(entry(&catalog, C, None)?, "description")?,
        v3::Text {
            key: Some("@desc_handler"),
            en: Some("Wanted [MISSING].\nReward on delivery."),
            de: None
        },
        "tokens are replaced, line breaks decoded and the blanks removed"
    );
    assert_eq!(
        projections
            .text(entry(&catalog, E, None)?, "description")?
            .en,
        Some("Haul it")
    );
    assert_eq!(
        projections.text(entry(&catalog, A, None)?, "description")?,
        v3::Text {
            key: None,
            en: None,
            de: None
        },
        "a contract that states none"
    );
    Ok(())
}
