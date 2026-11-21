// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, Research, ResearchValue};
use p4kx_extraction::DataForgeEntry;
use serde_json::{Value, json};

use crate::catalog_crafting_support::blueprint;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};
use crate::crafting_research_fixture as fixture;

#[test]
fn unsupported_research_graphs_keep_their_own_tier_and_original_errors() -> TestResult {
    let (_directory, archive) = common::open(&fixture::graph(), DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let data = p4kx_export::read_crafting(&forge, None, None)?;
    let [blueprint] = data.blueprints.as_slice() else {
        return Err("one original Blueprint".into());
    };
    let [empty, graph, known] = blueprint.tiers.as_slice() else {
        return Err("three original tiers".into());
    };
    for (tier, expected) in [
        (empty, ResearchValue::Null),
        (graph, ResearchValue::Unsupported(Some(fixture::UNKNOWN))),
        (known, ResearchValue::Present),
    ] {
        let Research::Present(block) = &tier.research else {
            return Err("present research envelope".into());
        };
        assert_eq!(
            block.research_costs.value, expected,
            "each tier has its own original state"
        );
    }
    let mut options = common::options();
    assert!(
        matches!(
            p4kx_export::build_catalog_from(&archive, &entry, &options),
            Err(CatalogError::IncompleteExport(_))
        ),
        "original failed reads cannot be published as a complete catalog"
    );
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog_from(&archive, &entry, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    check_graph(&catalog)?;
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

fn check_graph(catalog: &Value) -> TestResult {
    let recipe = blueprint::by_guid(catalog, fixture::OWNER)?;
    let tiers = recipe.tiers()?;
    let [empty, graph, known] = tiers.as_slice() else {
        return Err("three projected tier slots".into());
    };
    assert!(
        empty["source"] != graph["source"]
            && graph["source"] != known["source"]
            && empty["source"] != known["source"],
        "the tier projections point to distinct original source occurrences"
    );
    let path = "/research/envelope/value/researchCosts";
    for (tier, expected) in [
        (*empty, "null"),
        (*graph, "unsupported"),
        (*known, "present"),
    ] {
        assert_eq!(
            at(at(tier, path)?, "/value/state")?,
            expected,
            "authored tier order"
        );
    }
    let unsupported = recipe.node(at(at(graph, path)?, "/source")?)?;
    let missing = recipe.field(unsupported, "missing")?;
    assert_eq!(
        at(missing, "/originalError/type")?,
        "danglingReference",
        "missing target stays explicit"
    );
    assert_eq!(
        at(missing, "/originalError/guid/value")?,
        &json!(guid(fixture::MISSING)),
        "the original missing target GUID survives"
    );
    let next = recipe.field(unsupported, "next")?;
    let repeated = recipe.field(next, "next")?;
    assert_eq!(
        at(repeated, "/originalError/type")?,
        "ownedTwice",
        "the back edge retains its original error"
    );
    assert_eq!(
        at(repeated, "/value/state")?,
        "unreadable",
        "the cycle ends at the failed source slot"
    );
    assert_eq!(
        at(
            repeated,
            "/originalError/instanceIndex/containsEncryptedSource"
        )?,
        true,
        "the original failed pointer's index retains its encryption evidence"
    );
    Ok(())
}
