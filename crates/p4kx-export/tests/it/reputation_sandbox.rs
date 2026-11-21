// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as archive_fixture, DcbOrigin, TestResult, at, guid};

#[path = "reputation_sandbox_fixture.rs"]
mod fixture;

#[test]
fn sandbox_records_keep_the_original_owned_triggers_and_reward_amounts() -> TestResult {
    let (_folder, archive) = archive_fixture::open(&fixture::database(), DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &archive_fixture::options())?;
    let catalog = serde_json::from_slice(built.json())?;
    let trigger = at(&catalog, "/definitions/reputationTriggerRecords/rows/0")?;
    assert_eq!(at(trigger, "/guid/value")?, &json!(guid(fixture::RECORD)));
    let manual = at(trigger, "/triggerManualParams/value/0/value")?;
    assert_eq!(at(manual, "/kind")?, "manual");
    assert_eq!(at(manual, "/triggerOnInnocentsOnly/value")?, &json!(false));
    let infraction = at(manual, "/infractionTrigger/value")?;
    assert_eq!(at(infraction, "/kind")?, "definition");
    assert_eq!(
        at(
            infraction,
            "/sandboxInfractionDefinitionTrigger/value/guid/value"
        )?,
        &json!(guid(fixture::INFRACTION))
    );
    let outcome = at(manual, "/outcomes/value/0/value")?;
    assert_eq!(at(outcome, "/kind")?, "faction");
    assert_eq!(
        at(outcome, "/scope/value/guid/value")?,
        &json!(guid(fixture::SCOPE))
    );
    assert_eq!(
        at(outcome, "/reward/value/guid/value")?,
        &json!(guid(fixture::REWARD))
    );
    assert_eq!(
        at(outcome, "/factionReputation/value/guid/value")?,
        &json!(guid(fixture::MISSING_FACTION))
    );
    let reward = at(&catalog, "/definitions/reputationRewardAmounts/rows/0")?;
    assert_eq!(at(reward, "/guid/value")?, &json!(guid(fixture::REWARD)));
    assert_eq!(at(reward, "/editorName/value")?, "Fixture reward");
    assert_eq!(at(reward, "/reputationAmount/value")?, &json!(-32));
    assert_eq!(
        at(reward, "/reputationAmount/containsEncryptedSource")?,
        &json!(true)
    );
    assert_eq!(at(reward, "/reputationAmount/sourceSetComplete")?, true);
    assert!(
        !at(reward, "/reputationAmount/sourceUsesRefs")?
            .as_array()
            .ok_or("original reward references")?
            .is_empty(),
        "the signed value names its original physical source"
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    if let Err(error) = validator.validate(&catalog) {
        return Err(format!("{}: {}", error.instance_path(), error).into());
    }
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

#[test]
fn filtered_sandbox_targets_keep_declared_availability_and_original_guid() -> TestResult {
    let (_folder, archive) = archive_fixture::open(&fixture::database(), DcbOrigin::Plain)?;
    let filter = Filter::new("reputation/triggers");
    let mut options = archive_fixture::options();
    options.filter = Some(&filter);
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let reward_path = "/definitions/reputationRewardAmounts";
    let rewards = at(&catalog, reward_path)?;
    assert_eq!(at(rewards, "/availability")?, "available");
    assert_eq!(at(rewards, "/rows")?, &json!([]));
    assert_eq!(
        at(&catalog, "/completeness/reputationRewardAmounts/status")?,
        "partial"
    );
    let manual = at(
        &catalog,
        "/definitions/reputationTriggerRecords/rows/0/triggerManualParams/value/0/value",
    )?;
    let reward = at(manual, "/outcomes/value/0/value/reward")?;
    assert_eq!(
        at(reward, "/value/guid/value")?,
        &json!(guid(fixture::REWARD))
    );
    let links = at(&catalog, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or("original unresolved references")?;
    let original = json!(guid(fixture::REWARD));
    assert!(
        links.iter().any(|row| {
            row.pointer("/targetGuid/value") == Some(&original)
                && row["kind"] == "reputationTriggerRecords"
                && row["status"] == "excludedByFilter"
        }),
        "the omitted original reward remains auditable"
    );
    Ok(())
}

#[test]
fn faction_owns_ordered_trigger_slots_and_links_reusable_definitions() -> TestResult {
    let (_folder, archive) = archive_fixture::open(&fixture::database(), DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &archive_fixture::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let faction = at(&catalog, "/definitions/factionReputations/rows/0")?;
    let configuration = at(&catalog, "/configurations/reputationTriggers/rows/0")?;
    assert_eq!(
        at(configuration, "/owner/guid/value")?,
        &json!(guid(fixture::OWNER))
    );
    assert_eq!(
        at(configuration, "/owner/recordIndex/value")?,
        at(faction, "/recordIndex/value")?,
        "both tables preserve the same physical owner"
    );
    let empty = at(&catalog, "/configurations/reputationTriggers/rows/1")?;
    assert_eq!(
        at(empty, "/owner/guid/value")?,
        &json!(guid(fixture::EMPTY_OWNER))
    );
    assert_eq!(at(empty, "/sandboxTriggers/value")?, &json!([]));

    assert_trigger_slots(&catalog, configuration)?;
    assert!(
        !at(faction, "/authoredFields/roots")?
            .as_array()
            .ok_or("remaining authored fields")?
            .iter()
            .any(|field| field.pointer("/name/value") == Some(&json!("sandboxTriggers"))),
        "the owner does not duplicate the represented array"
    );
    assert_eq!(
        at(&catalog, "/definitions/reputationTriggerRecords/rows")?
            .as_array()
            .ok_or("reusable trigger records")?
            .len(),
        1,
        "references do not materialize another definition"
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    if let Err(error) = validator.validate(&catalog) {
        return Err(format!("{}: {}", error.instance_path(), error).into());
    }
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

fn assert_trigger_slots(catalog: &Value, configuration: &Value) -> TestResult {
    let triggers = at(configuration, "/sandboxTriggers/value")?
        .as_array()
        .ok_or("original trigger occurrences")?;
    let [manual, null, linked, future] = triggers.as_slice() else {
        return Err("four original trigger positions".into());
    };
    assert_eq!(
        at(manual, "/value/kind")?,
        "manual",
        "the first slot is the manual trigger"
    );
    assert_eq!(
        at(manual, "/value/triggerOnInnocentsOnly/value")?,
        true,
        "the authored victim filter survives"
    );
    assert_eq!(
        at(
            manual,
            "/value/infractionTrigger/value/sandboxTrigger/value"
        )?,
        fixture::EVENT_LABEL,
        "the authored event label is open vocabulary"
    );
    assert_eq!(
        at(
            manual,
            "/value/outcomes/value/1/value/factionReputation/value/guid/value"
        )?,
        &json!(guid(fixture::OWNER)),
        "the link back to the same faction owner remains a reference"
    );
    assert_authored_variant(catalog, future)?;
    assert_eq!(
        at(null, "/state")?,
        "null",
        "the null pointer stays in its original slot"
    );
    assert_eq!(
        at(null, "/containsEncryptedSource")?,
        true,
        "the null pointer has its own provenance"
    );
    assert_eq!(
        at(linked, "/value/kind")?,
        "record",
        "the third slot links a reusable Record"
    );
    assert_eq!(
        at(linked, "/value/triggerRecord/value/guid/value")?,
        &json!(guid(fixture::RECORD)),
        "the link retains the original target GUID"
    );
    assert_eq!(
        at(linked, "/value/triggerRecord/containsEncryptedSource")?,
        true,
        "the link carries immediate source provenance"
    );
    Ok(())
}

fn assert_authored_variant(catalog: &Value, carrier: &Value) -> TestResult {
    let variant = at(carrier, "/value")?;
    assert_eq!(
        at(variant, "/kind")?,
        "authored",
        "the unclassified variant uses the original-value branch"
    );
    assert_eq!(
        at(variant, "/sourceType/value")?,
        fixture::FUTURE,
        "exact concrete source type"
    );
    assert_eq!(
        at(variant, "/authoredFields/roots/0/name/value")?,
        "predicate",
        "original root predicate field"
    );
    let values = at(variant, "/authoredFields/values")?
        .as_array()
        .ok_or("original nested predicate values")?;
    let [predicate, negated] = values.as_slice() else {
        return Err("the unclassified nested condition is retained in full".into());
    };
    assert_eq!(
        at(predicate, "/value/value/fields/0/name/value")?,
        "negated",
        "original nested negation field"
    );
    assert_eq!(
        at(negated, "/value/value/type")?,
        "boolean",
        "original predicate storage type"
    );
    assert_eq!(
        at(negated, "/value/value/value")?,
        true,
        "original negated flag"
    );
    assert_eq!(
        at(negated, "/value/containsEncryptedSource")?,
        true,
        "immediate provenance of the nested flag"
    );
    let diagnostics = at(catalog, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("unknown trigger coverage diagnostics")?;
    assert!(
        diagnostics.iter().any(|row| {
            row["kind"] == "factionReputations"
                && row["code"] == "unsupportedValue"
                && row["severity"] == "information"
        }),
        "the retained variant remains visible in export coverage"
    );
    Ok(())
}
