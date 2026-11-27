// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original Broker definitions and contextual projections in the catalog.

use serde_json::{Value, json};

use crate::broker_flow_graph as broker;
use crate::catalog_contract_templates::build;
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::mission_template_graph as mission;
use crate::template_canonical_graph::{BrokerContext, Context};
use crate::template_fixture_fields as fields;
use crate::template_fixture_tags as tags;
use crate::template_fixture_text as texts;

#[path = "catalog_broker_checks.rs"]
mod checks;
#[path = "catalog_broker_entries_reputation.rs"]
mod reputation;
#[path = "catalog_broker_entries_context.rs"]
mod template_context;

const BROKER: &str = "b0000000-0000-4000-8000-000000000001";
const ELSEWHERE: &str = "b0000000-0000-4000-8000-000000000002";
const MISSION_TYPE: &str = "50000000-0000-4000-8000-000000000001";
const OWNER: &str = "80000000-0000-4000-8000-000000000001";
const SCOPE: &str = "80000000-0000-4000-8000-000000000002";
const REWARD: &str = "b1000000-0000-4000-8000-000000000001";

#[test]
fn an_entry_outside_the_folder_is_kept_a_leftover_is_left_out_and_a_contract_links_to_its_entry()
-> TestResult {
    let catalog = build()?;
    let definitions = catalog
        .pointer("/definitions/missionBrokerEntries/rows")
        .and_then(Value::as_array)
        .ok_or("actual canonical Broker definitions")?;
    let expected = [
        crate::spawns_fixture::BROKER,
        crate::spawns_fixture::BROKER_ELSEWHERE,
        crate::spawns_fixture::BROKER_BAD,
        crate::spawns_fixture::BROKER_BAD_PROPERTY,
        crate::spawns_fixture::BROKER_BAD_LOCATION,
    ]
    .map(crate::commodity_v3_fixture::guid);
    assert_eq!(
        definitions.len(),
        expected.len(),
        "readable original Broker owners remain in D, and the leftover is excluded"
    );
    for (row, identity) in definitions.iter().zip(expected) {
        assert_eq!(
            row.pointer("/guid/value"),
            Some(&json!(identity)),
            "ordered by original GUID"
        );
    }
    assert_eq!(
        broker::projections(&catalog)?.len(),
        2,
        "only the two successful contexts are emitted"
    );
    broker::projection(&catalog, broker::definition(&catalog, ELSEWHERE)?)?;
    for contract in crate::catalog_contracts::v3::rows(&catalog)? {
        assert_eq!(
            contract.get("brokerEntryGuid"),
            Some(&json!(BROKER)),
            "the same original contract link"
        );
    }
    let definition = broker::definition(&catalog, BROKER)?;
    broker::projection(&catalog, definition)?;
    assert_eq!(
        definition.pointer("/className/value"),
        Some(&json!("Haul_A")),
        "the same original header conversion"
    );
    assert_eq!(
        definition.pointer("/recordPath/value"),
        Some(&json!("libs/foundry/records/missionbroker/covalex/haul_a")),
        "the same original path conversion"
    );
    Ok(())
}

#[test]
fn texts_are_game_texts_and_a_placeholder_has_no_key() -> TestResult {
    let catalog = build()?;
    let graph = BrokerContext::new(&catalog, BROKER)?;
    let title = graph.owner().follow(
        graph
            .projection()
            .get("title")
            .ok_or("actual title source link")?,
    )?;
    assert!(
        std::ptr::eq(title, graph.root("title")?),
        "context points to the actual original title"
    );
    let deadline = graph.root("missionDeadline")?;
    for node in [
        title,
        graph.root("missionGiver")?,
        graph.field(deadline, "missionEndReason")?,
    ] {
        texts::localized(
            &catalog,
            &graph,
            node,
            "@place_name",
            Some("Hijacked Ship"),
            None,
        )?;
    }
    texts::no_lookup(
        &graph,
        graph.root("titleHUD")?,
        "@LOC_UNINITIALIZED",
        "placeholder",
    )?;
    assert_eq!(
        fields::text(graph.root("playerFacingDebugName")?)?,
        "Haul A",
        "exact original debug name"
    );
    Ok(())
}

#[test]
fn links_and_the_identity_of_the_giver_are_guids() -> TestResult {
    let catalog = build()?;
    let graph = BrokerContext::new(&catalog, BROKER)?;
    mission::reference(graph.root("type")?, MISSION_TYPE, None)?;
    for (name, expected) in [
        ("locationMissionAvailable", OWNER),
        ("invitationMission", ELSEWHERE),
    ] {
        fields::reference(graph.root(name)?, expected)?;
    }
    for name in ["localityAvailable", "linkedMission"] {
        raw::source_null(graph.root(name)?);
    }
    for name in ["associatedMissions", "requiredMissions"] {
        let values = fields::elements(&graph, graph.root(name)?)?;
        let [value] = values.as_slice() else {
            return Err("one actual original linked mission".into());
        };
        fields::reference(value, ELSEWHERE)?;
    }
    let owner = graph
        .projection()
        .get("owner")
        .ok_or("actual selected DevOwner context")?;
    let caller = graph
        .owner()
        .follow(owner.get("source").ok_or("actual original owner slot")?)?;
    assert!(
        std::ptr::eq(caller, graph.root("owner")?),
        "the same original caller reference"
    );
    fields::reference(caller, OWNER)?;
    assert_eq!(
        owner.pointer("/className/value"),
        Some(&json!("MDT")),
        "the actual selected original owner class"
    );
    for (name, expected) in [
        ("missionGiverFragmentTags", "Covalex"),
        ("missionModule", "Libs/Subsumption/Missions/Haul.xml"),
    ] {
        assert_eq!(
            fields::text(graph.root(name)?)?,
            expected,
            "exact original text {name}"
        );
    }
    Ok(())
}

#[test]
fn terms_are_exported_as_stored() -> TestResult {
    let catalog = build()?;
    let graph = BrokerContext::new(&catalog, BROKER)?;
    for (name, expected) in [
        ("refundBuyInOnWithdraw", true),
        ("canBeShared", true),
        ("onlyOwnerCanComplete", true),
        ("initiallyActive", true),
        ("requestOnly", false),
        ("failIfBecameCriminal", true),
    ] {
        assert_eq!(
            fields::scalar(graph.root(name)?, "boolean")?,
            &json!(expected),
            "exact original flag {name}"
        );
    }
    for (name, expected) in [
        ("missionDifficulty", json!(3)),
        ("missionBuyInAmount", json!(500)),
        ("personalCooldownTime", json!(60.0)),
        ("respawnTime", json!(10.0)),
        ("maxInstances", json!(4)),
        ("instanceLifeTime", json!(120.0)),
    ] {
        assert_eq!(
            fields::number(graph.root(name)?)?,
            &expected,
            "exact original numeric field {name}"
        );
    }
    let wanted = graph.field(graph.root("reputationPrerequisites")?, "wantedLevel")?;
    fields::numeric_fields(&graph, wanted, &[("maxValue", json!(2.0))])?;
    let reward = graph.root("missionReward")?;
    assert_eq!(
        fields::text(graph.field(reward, "currencyType")?)?,
        "UEC",
        "the complete original currency"
    );
    fields::numeric_fields(
        &graph,
        reward,
        &[("reward", json!(12_000)), ("max", json!(15_000))],
    )?;
    fields::scalar_fields(&graph, reward, &[("plusBonuses", "boolean", json!(true))])?;
    mission::reference(graph.field(reward, "reputationBonus")?, SCOPE, None)?;
    let finish = graph.root("missionDeadline")?;
    fields::scalar_fields(
        &graph,
        finish,
        &[("missionAutoEnd", "boolean", json!(true))],
    )?;
    fields::numeric_fields(&graph, finish, &[("missionCompletionTime", json!(45.5))])?;
    let state = graph.field(finish, "missionResultAfterTimerEnd")?;
    assert_eq!(fields::text(state)?, "Failed", "the exact stored result");
    assert_eq!(
        fields::vocabulary(&graph, state)?.get("value"),
        Some(&json!("failed")),
        "the same recognized timer result"
    );
    Ok(())
}

#[test]
fn the_schedule_names_its_dates_and_nulls_a_month_it_does_not_know() -> TestResult {
    let catalog = build()?;
    let graph = BrokerContext::new(&catalog, BROKER)?;
    let windows = fields::elements(&graph, graph.root("availableDateSchedule")?)?;
    let [window] = windows.as_slice() else {
        return Err("the one actual original date window".into());
    };
    for (name, year, month, day, hour) in [
        ("start", 2950, ("December", "december"), 1, 8),
        ("end", 2951, ("January", "january"), 8, 17),
    ] {
        let stamp = graph.field(window, name)?;
        let date = graph.field(stamp, "date")?;
        fields::numeric_fields(&graph, date, &[("year", json!(year)), ("day", json!(day))])?;
        vocabulary(
            &graph,
            graph.field(date, "month")?,
            month.0,
            &json!(month.1),
        )?;
        let time = graph.field(stamp, "time")?;
        fields::numeric_fields(
            &graph,
            time,
            &[
                ("hour", json!(hour)),
                ("minute", json!(30)),
                ("second", json!(0)),
            ],
        )?;
    }
    let repeat = graph.field(window, "repeatEnd")?;
    fields::numeric_fields(&graph, repeat, &[("year", json!(2999)), ("day", json!(1))])?;
    vocabulary(
        &graph,
        graph.field(repeat, "month")?,
        "Smarch",
        &Value::Null,
    )?;
    for name in ["hour", "minute", "second"] {
        fields::absent_field(repeat, name)?;
    }
    vocabulary(
        &graph,
        graph.field(window, "repeatType")?,
        "RepeatYearly",
        &json!("repeatYearly"),
    )
}

fn vocabulary(graph: &BrokerContext<'_>, node: &Value, text: &str, meaning: &Value) -> TestResult {
    assert_eq!(
        fields::text(node)?,
        text,
        "the complete original vocabulary label"
    );
    assert_eq!(
        fields::vocabulary(graph, node)?.get("value"),
        Some(meaning),
        "the established recognized or unknown meaning remains separate"
    );
    Ok(())
}

#[test]
fn tags_are_paths_and_required_missions_are_terms() -> TestResult {
    let catalog = build()?;
    let graph = BrokerContext::new(&catalog, BROKER)?;
    tag_list(
        &graph,
        graph.field(graph.root("completionTags")?, "tags")?,
        "Kill",
    )?;
    tag_list(&graph, graph.root("missionTags")?, "Missions.Hijacked")?;
    assert!(
        raw::elements(graph.field(graph.root("requiredAreaTags")?, "tags")?)?.is_empty(),
        "the original area tags are empty"
    );
    let terms = fields::elements(&graph, graph.root("requiredCompletedMissionTags")?)?;
    let [term] = terms.as_slice() else {
        return Err("the one actual original completed-mission term".into());
    };
    tag_list(&graph, graph.field(term, "positiveTags")?, "Kill")?;
    tag_list(
        &graph,
        graph.field(term, "negativeTags")?,
        "Missions.Hijacked",
    )?;
    assert_eq!(
        raw::elements(graph.root("journalEntriesToAddOnComplete")?)?.len(),
        1,
        "the original journal-entry cardinality remains exact"
    );
    Ok(())
}

fn tag_list(graph: &BrokerContext<'_>, node: &Value, path: &str) -> TestResult {
    let original = fields::elements(graph, node)?;
    let [tag] = original.as_slice() else {
        return Err("the one original tag reference".into());
    };
    tags::path(graph, tag, Some(path))
}
