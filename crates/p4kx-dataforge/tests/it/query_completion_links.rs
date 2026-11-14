// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Completion after `->` offers the fields of the struct a Reference or weak pointer declares.

use std::error::Error;

use crate::fleet::{database, rows};
use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::QuerySchema;

type TestResult = Result<(), Box<dyn Error>>;

fn names(schema: &QuerySchema, text: &str) -> Vec<String> {
    schema
        .complete(text, text.len())
        .map_or_default(|completion| {
            completion
                .candidates()
                .iter()
                .map(|candidate| candidate.name().to_owned())
                .collect()
        })
}

#[test]
fn an_arrow_offers_the_fields_of_the_linked_struct_also_chained() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let schema = QuerySchema::of(&forge)?;

    assert_eq!(
        names(&schema, "from Ship select maker->"),
        ["home", "name"],
        "Reference"
    );
    assert_eq!(
        names(&schema, "from Ship select maker->home->"),
        ["mass", "name"],
        "chained"
    );
    assert_eq!(
        names(&schema, "from Ship select pilot->"),
        ["role"],
        "weak pointer"
    );
    assert_eq!(
        names(&schema, "from Ship select escorts[*]->na"),
        ["name"],
        "behind elements"
    );
    assert_eq!(
        names(&schema, "from Ship where shield->"),
        ["name"],
        "in a condition"
    );
    Ok(())
}

#[test]
fn an_arrow_offers_nothing_behind_an_untyped_link_or_a_value() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let schema = QuerySchema::of(&forge)?;

    for text in [
        "from Ship select plain->",
        "from Ship select mass->",
        "from Ship select escorts->",
        "from Ship select maker.",
    ] {
        assert!(schema.complete(text, text.len()).is_none(), "{text}");
    }
    Ok(())
}

#[test]
fn a_link_field_shows_its_target_as_detail() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let schema = QuerySchema::of(&forge)?;
    let completion = schema
        .complete("from Ship select ", 17)
        .ok_or("no completion")?;

    let detail = |name: &str| {
        completion
            .candidates()
            .iter()
            .find(|candidate| candidate.name() == name)
            .and_then(|candidate| candidate.detail())
            .map(str::to_owned)
    };

    assert_eq!(
        detail("maker").as_deref(),
        Some("Reference Manufacturer"),
        "Reference"
    );
    assert_eq!(
        detail("escorts").as_deref(),
        Some("Reference Ship[]"),
        "array"
    );
    assert_eq!(detail("plain").as_deref(), Some("Reference"), "untyped");
    assert_eq!(
        rows(&forge, "from Ship select maker->name")?.len(),
        3,
        "the fixture runs"
    );
    Ok(())
}
