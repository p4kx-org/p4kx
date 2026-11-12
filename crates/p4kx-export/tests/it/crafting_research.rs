// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "crafting_research_graph.rs"]
mod graph;

use p4kx_export::{CatalogError, CatalogPublicationMode, Research, ResearchValue};
use p4kx_extraction::{DataForgeEntry, DataType, Value as Raw};
use serde_json::Value;

use crate::catalog_crafting_support::blueprint;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::crafting_research_fixture::{self as fixture, Case};

#[test]
fn blueprint_research_distinguishes_absent_null_empty_and_known() -> TestResult {
    for (case, state) in [
        (Case::Absent, "absent"),
        (Case::Null, "null"),
        (Case::Empty, "present"),
        (Case::Known, "present"),
    ] {
        let database = fixture::database(case);
        for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
            let (_directory, archive) = common::open(&database, origin)?;
            let entry = DataForgeEntry::read(&archive)?;
            let forge = entry.open()?;
            let record = forge.records().next().ok_or("original Blueprint")?;
            let tree = record.tree()?;
            assert!(tree.is_from(&record));
            assert_original(&tree, case)?;
            let data = p4kx_export::read_crafting(&forge, None, None)?;
            let [blueprint] = data.blueprints.as_slice() else {
                return Err("one standalone Blueprint".into());
            };
            let [tier] = blueprint.tiers.as_slice() else {
                return Err("one standalone tier".into());
            };
            assert_standalone(&tier.research, case)?;
            let built = p4kx_export::build_catalog_from(&archive, &entry, &common::options())?;
            assert!(built.summary().is_complete());
            let catalog: Value = serde_json::from_slice(built.json())?;
            assert_wire(&catalog, state, case, encrypted)?;
        }
    }
    Ok(())
}

#[test]
fn unsupported_or_unreadable_research_retains_exact_original_slots() -> TestResult {
    for (case, state) in [
        (Case::Unsupported, "unsupported"),
        (Case::Unreadable, "unreadable"),
    ] {
        let database = fixture::database(case);
        let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
        let entry = DataForgeEntry::read(&archive)?;
        let forge = entry.open()?;
        let record = forge.records().next().ok_or("original Blueprint")?;
        assert_original(&record.tree()?, case)?;
        assert!(
            matches!(
                p4kx_export::build_catalog_from(&archive, &entry, &common::options()),
                Err(CatalogError::IncompleteExport(_))
            ),
            "only explicit diagnostic publication may include the unsupported interpretation"
        );
        let mut options = common::options();
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        let built = p4kx_export::build_catalog_from(&archive, &entry, &options)?;
        assert!(!built.summary().is_complete());
        let catalog: Value = serde_json::from_slice(built.json())?;
        assert_wire(&catalog, "present", case, true)?;
        let recipe = blueprint::by_guid(&catalog, fixture::OWNER)?;
        let tiers = recipe.tiers()?;
        let tier = tiers.first().ok_or("actual research tier")?;
        let costs = &tier["research"]["envelope"]["value"]["researchCosts"];
        assert_eq!(costs["value"]["state"], state);
        let raw = recipe.node(&costs["source"])?;
        match case {
            Case::Unsupported => {
                let token = recipe.field(raw, "opaqueToken")?;
                assert_eq!(
                    token["value"]["value"]["value"]["value"],
                    "18446744073709551615"
                );
                assert_eq!(token["value"]["containsEncryptedSource"], true);
            }
            Case::Unreadable => {
                assert_eq!(raw["value"]["state"], "unreadable");
                assert!(
                    !raw["originalError"].is_null(),
                    "the exact original read error is retained"
                );
            }
            _ => return Err("invalid failure control".into()),
        }
    }
    Ok(())
}

fn assert_original(tree: &p4kx_extraction::RecordTree<'_>, case: Case) -> TestResult {
    let Some(Raw::Struct(blueprint)) = tree.root().field("blueprint") else {
        return Err("original body".into());
    };
    let Some(Raw::Array(tiers)) = blueprint.field("tiers") else {
        return Err("original tiers".into());
    };
    let [Raw::Struct(tier)] = tiers.as_ref() else {
        return Err("one original tier".into());
    };
    match (case, tier.field("research")) {
        (Case::Absent, None) | (Case::Null, Some(Raw::Null)) => Ok(()),
        (_, Some(Raw::Struct(research))) => {
            assert_eq!(
                research.type_name(),
                fixture::RESEARCH,
                "the original research envelope type"
            );
            let cost = research
                .fields()
                .iter()
                .find(|field| field.name() == "researchCosts")
                .ok_or("actual costs declaration")?;
            assert_eq!(
                cost.storage().data_type(),
                DataType::StrongPointer,
                "the declaration retains its actual pointer storage"
            );
            assert!(
                !cost.storage().is_array(),
                "the research costs slot is scalar"
            );
            assert_eq!(
                research.field("unlockRequirements"),
                Some(&Raw::Null),
                "the original unlock pointer is explicitly null"
            );
            match (case, cost.value()) {
                (Case::Empty, Raw::Null) | (Case::Unreadable, Raw::Error(_)) => Ok(()),
                (Case::Known, Raw::Struct(base))
                    if base.type_name() == fixture::COSTS && base.fields().is_empty() =>
                {
                    Ok(())
                }
                (Case::Unsupported, Raw::Struct(base)) if base.type_name() == fixture::UNKNOWN => {
                    assert_eq!(
                        base.field("opaqueToken"),
                        Some(&Raw::UInt64(u64::MAX)),
                        "the unsupported original payload remains exactly readable"
                    );
                    Ok(())
                }
                _ => Err("unexpected original member".into()),
            }
        }
        _ => Err("unexpected original research envelope".into()),
    }
}

fn assert_standalone(research: &Research<'_>, case: Case) -> TestResult {
    match (case, research) {
        (Case::Absent, Research::Absent) | (Case::Null, Research::Null) => Ok(()),
        (Case::Empty | Case::Known, Research::Present(block)) => {
            let expected = if matches!(case, Case::Known) {
                ResearchValue::Present
            } else {
                ResearchValue::Null
            };
            assert_eq!(
                block.research_costs.value, expected,
                "standalone costs preserve their source state"
            );
            assert_eq!(
                block.unlock_requirements.value,
                ResearchValue::Null,
                "standalone unlocks preserve explicit null"
            );
            Ok(())
        }
        _ => Err("unexpected standalone research shape".into()),
    }
}

fn assert_wire(catalog: &Value, state: &str, case: Case, encrypted: bool) -> TestResult {
    let blueprint = blueprint::by_guid(catalog, fixture::OWNER)?;
    let tiers = blueprint.tiers()?;
    let tier = tiers.first().ok_or("one actual tier")?;
    let research = &tier["research"];
    let envelope = &research["envelope"];
    assert_eq!(
        envelope["state"], state,
        "the canonical envelope preserves source state"
    );
    assert_eq!(
        envelope["containsEncryptedSource"], encrypted,
        "the canonical envelope preserves source encryption"
    );
    if state == "absent" {
        assert!(
            research["source"].is_null(),
            "an absent envelope has no occurrence"
        );
        return Ok(());
    }
    let original = blueprint.node(&research["source"])?;
    if state == "null" {
        assert_eq!(
            common::at(original, "/value/state")?,
            "null",
            "a present null is retained in the source arena"
        );
        return Ok(());
    }
    assert_present_research(original, envelope, case, encrypted)
}

fn assert_present_research(
    original: &Value,
    envelope: &Value,
    case: Case,
    encrypted: bool,
) -> TestResult {
    assert_eq!(
        common::at(original, "/value/value/sourceType/value")?,
        fixture::RESEARCH,
        "the original arena retains the research type"
    );
    for (name, expected) in [
        ("researchCosts", fixture::COSTS),
        ("unlockRequirements", fixture::UNLOCK),
    ] {
        let value = envelope
            .get("value")
            .and_then(|value| value.get(name))
            .ok_or("projected research member")?;
        assert_eq!(
            value.get("source"),
            Some(blueprint::field_ref(original, name)?),
            "each projection references the exact authored member"
        );
        assert_eq!(
            common::at(value, "/declaredBase/value")?,
            expected,
            "the declared base name survives for each member"
        );
        assert_eq!(
            common::at(value, "/declaredBase/containsEncryptedSource")?,
            encrypted,
            "each base name retains its original encryption status"
        );
    }
    let costs = common::at(envelope, "/value/researchCosts")?;
    match case {
        Case::Empty => assert_eq!(
            common::at(costs, "/value/state")?,
            "null",
            "an empty costs pointer is explicitly null"
        ),
        Case::Known => {
            assert_eq!(
                common::at(costs, "/value/state")?,
                "present",
                "a known costs pointer is present"
            );
            assert_eq!(
                common::at(costs, "/value/value")?,
                costs.get("source").ok_or("original costs reference")?,
                "a known costs pointer links the exact original occurrence"
            );
        }
        _ => {}
    }
    Ok(())
}

#[test]
fn a_null_research_pointer_to_another_declared_family_is_unsupported() -> TestResult {
    let database = fixture::database(Case::WrongDeclaration);
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let data = p4kx_export::read_crafting(&forge, None, None)?;
    let [blueprint] = data.blueprints.as_slice() else {
        return Err("one original Blueprint".into());
    };
    let [tier] = blueprint.tiers.as_slice() else {
        return Err("one original tier".into());
    };
    let Research::Present(block) = &tier.research else {
        return Err("original research envelope".into());
    };
    assert_eq!(
        block.research_costs.value,
        ResearchValue::Unsupported(None),
        "a null payload cannot establish the expected declared base"
    );

    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog_from(&archive, &entry, &options)?;
    assert!(
        !built.summary().is_complete(),
        "the wrong declaration has an explicit coverage failure"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    let recipe = blueprint::by_guid(&catalog, fixture::OWNER)?;
    let tiers = recipe.tiers()?;
    let tier = tiers.first().ok_or("original research tier")?;
    let costs = &tier["research"]["envelope"]["value"]["researchCosts"];
    assert_eq!(
        costs["declaredBase"]["value"],
        fixture::UNLOCK,
        "the catalog retains the actual declaration"
    );
    assert_eq!(
        costs["value"]["state"], "unsupported",
        "the catalog and standalone reader classify the same declaration"
    );
    assert_eq!(
        recipe.node(&costs["source"])?["value"]["state"],
        "null",
        "the raw original pointer still records the null payload"
    );
    Ok(())
}
