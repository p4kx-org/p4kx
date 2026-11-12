// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, TreeBuilder, Value as Original, ValueError,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::catalog_v3_consumer as consumer;
use crate::real_archive_verification::print_line;

const FACTION: &str = "FactionReputation";
const TRIGGER: &str = "SandboxTriggerRecord";
const REWARD: &str = "SReputationRewardAmount";

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn sandbox_owners_and_definitions_match_original_records() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build sandbox reputation catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;

    let owner_rows = rows(&catalog, "/configurations/reputationTriggers/rows")?;
    let record_rows = rows(&catalog, "/definitions/reputationTriggerRecords/rows")?;
    let reward_rows = rows(&catalog, "/definitions/reputationRewardAmounts/rows")?;
    let owner_slots = check_family(&forge, owner_rows, FACTION, Some("sandboxTriggers"))?;
    let record_slots = check_family(&forge, record_rows, TRIGGER, Some("triggerManualParams"))?;
    check_family(&forge, reward_rows, REWARD, None)?;
    assert!(
        owner_slots + record_slots > 0,
        "Build contains sandbox trigger slots"
    );
    watch
        .check()
        .map_err(|_| "Archive changed during sandbox verification")?;
    print_line(&format!(
        "reputation sandbox: {owner_slots} owner slots, {record_slots} definition slots and {} rewards checked",
        reward_rows.len()
    ));
    Ok(())
}

fn rows<'catalog>(catalog: &'catalog Value, path: &str) -> TestResult<&'catalog [Value]> {
    catalog
        .pointer(path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "expected sandbox catalog table".into())
}

fn check_family(
    forge: &DataForge<'_>,
    rows: &[Value],
    source_type: &str,
    list: Option<&str>,
) -> TestResult<usize> {
    let expected: BTreeSet<_> = forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == source_type))
        .map(|record| record.index())
        .collect();
    assert!(
        !expected.is_empty(),
        "configured Build contains the source family"
    );
    let mut seen = BTreeSet::new();
    let mut trees = TreeBuilder::new();
    let mut slots = 0;
    for row in rows {
        let header = if source_type == FACTION {
            row.get("owner").ok_or("sandbox configuration owner")?
        } else {
            row
        };
        let index = u32::try_from(
            header
                .pointer("/recordIndex/value")
                .and_then(Value::as_u64)
                .ok_or("physical sandbox source index")?,
        )?;
        assert!(
            expected.contains(&index) && seen.insert(index),
            "one row per physical sandbox source"
        );
        let record = forge.record(index).ok_or("original sandbox Record")?;
        assert!(
            header.pointer("/guid/value").and_then(Value::as_str)
                == Some(record.guid().to_string().as_str()),
            "the original sandbox GUID is retained"
        );
        let tree = trees
            .build(&record)
            .map_err(|_| "read original sandbox Record")?;
        if let Some(name) = list {
            slots += check_slots(
                row.get(name).ok_or("sourced sandbox array")?,
                tree.field(name),
            )?;
        } else {
            check_reward(
                row,
                tree.field("reputationAmount"),
                tree.field("editorName"),
            )?;
        }
    }
    assert_eq!(
        seen.len(),
        expected.len(),
        "every sandbox Record is represented"
    );
    Ok(slots)
}

fn check_reward(
    row: &Value,
    amount: Option<&Original<'_>>,
    name: Option<&Original<'_>>,
) -> TestResult {
    let Some(Original::Int32(amount)) = amount else {
        return Err("missing original signed reward amount".into());
    };
    let Some(Original::String(name)) = name else {
        return Err("missing original reward editor name".into());
    };
    assert!(
        row.pointer("/reputationAmount/value")
            .and_then(Value::as_i64)
            == Some(i64::from(*amount)),
        "original signed reputation amount"
    );
    assert!(
        row.pointer("/editorName/value").and_then(Value::as_str) == Some(name),
        "original reward editor name"
    );
    Ok(())
}

fn check_slots(carrier: &Value, original: Option<&Original<'_>>) -> TestResult<usize> {
    let Some(Original::Array(originals)) = original else {
        return check_state(carrier, original).map(|()| 0);
    };
    let exported = consumer::source_value(carrier)?
        .and_then(Value::as_array)
        .ok_or("original sandbox array has an exported value")?;
    assert_eq!(
        exported.len(),
        originals.len(),
        "original sandbox slot count"
    );
    for (cell, original) in exported.iter().zip(originals.iter()) {
        let Original::Struct(body) = original else {
            check_state(cell, Some(original))?;
            continue;
        };
        let value = consumer::source_value(cell)?.ok_or("sourced sandbox slot")?;
        assert!(
            value.pointer("/sourceType/value").and_then(Value::as_str) == Some(body.type_name()),
            "original sandbox variant at the same position"
        );
        match body.type_name() {
            "SandboxTriggerManualParams" => {
                let Some(Original::Bool(flag)) = body.field("triggerOnInnocentsOnly") else {
                    return Err("missing original sandbox victim flag".into());
                };
                assert_eq!(
                    value
                        .pointer("/triggerOnInnocentsOnly/value")
                        .and_then(Value::as_bool),
                    Some(*flag),
                    "original sandbox victim flag"
                );
                check_infraction(
                    value
                        .get("infractionTrigger")
                        .ok_or("sourced infraction trigger")?,
                    body.field("infractionTrigger"),
                )?;
                check_outcomes(
                    value.get("outcomes").ok_or("sourced sandbox outcomes")?,
                    body.field("outcomes"),
                )?;
            }
            "SandboxTriggerRecordParams" => {
                check_reference(
                    value
                        .get("triggerRecord")
                        .ok_or("sourced trigger Record link")?,
                    body.field("triggerRecord"),
                )?;
            }
            _ => assert_eq!(
                value.get("kind").and_then(Value::as_str),
                Some("authored"),
                "unclassified trigger remains visible"
            ),
        }
    }
    Ok(originals.len())
}

fn check_infraction(carrier: &Value, original: Option<&Original<'_>>) -> TestResult {
    let Some(Original::Struct(body)) = original else {
        return check_state(carrier, original);
    };
    let value = consumer::source_value(carrier)?.ok_or("sourced infraction trigger")?;
    assert!(
        value.pointer("/sourceType/value").and_then(Value::as_str) == Some(body.type_name()),
        "original infraction variant"
    );
    match body.type_name() {
        "SandboxTrigger" => {
            let Some(Original::Enum(event)) = body.field("sandboxTrigger") else {
                return Err("missing original infraction event".into());
            };
            assert!(
                value
                    .pointer("/sandboxTrigger/value")
                    .and_then(Value::as_str)
                    == Some(event),
                "original infraction event label"
            );
        }
        "SandboxInfractionDefinitionTrigger" => {
            let name = "sandboxInfractionDefinitionTrigger";
            check_reference(
                value
                    .get(name)
                    .ok_or("sourced infraction definition link")?,
                body.field(name),
            )?;
        }
        _ => assert_eq!(
            value.get("kind").and_then(Value::as_str),
            Some("authored"),
            "unclassified infraction remains visible"
        ),
    }
    Ok(())
}

fn check_outcomes(carrier: &Value, original: Option<&Original<'_>>) -> TestResult {
    let Some(Original::Array(originals)) = original else {
        return check_state(carrier, original);
    };
    let exported = consumer::source_value(carrier)?
        .and_then(Value::as_array)
        .ok_or("sourced outcome array")?;
    assert_eq!(exported.len(), originals.len(), "original outcome count");
    for (cell, original) in exported.iter().zip(originals.iter()) {
        let Original::Struct(body) = original else {
            check_state(cell, Some(original))?;
            continue;
        };
        let value = consumer::source_value(cell)?.ok_or("sourced outcome")?;
        assert!(
            value.pointer("/sourceType/value").and_then(Value::as_str) == Some(body.type_name()),
            "original outcome type"
        );
        for name in ["scope", "reward", "factionReputation"] {
            if let Some(field) = body.field(name) {
                check_reference(value.get(name).ok_or("sourced outcome link")?, Some(field))?;
            }
        }
    }
    Ok(())
}

fn check_reference(carrier: &Value, original: Option<&Original<'_>>) -> TestResult {
    let (guid, status) = match original {
        Some(Original::Reference(target)) => (target.guid(), "present"),
        Some(Original::Error(ValueError::DanglingReference { guid })) => (*guid, "dangling"),
        _ => return check_state(carrier, original),
    };
    let guid = guid.to_string();
    assert!(
        carrier.pointer("/value/guid/value").and_then(Value::as_str) == Some(guid.as_str())
            && carrier.pointer("/value/status").and_then(Value::as_str) == Some(status),
        "exact original sandbox link"
    );
    Ok(())
}

fn check_state(carrier: &Value, original: Option<&Original<'_>>) -> TestResult {
    let state = carrier.get("state").and_then(Value::as_str);
    let matches_source = match original {
        None => state == Some("absent"),
        Some(Original::Null) => state == Some("null"),
        Some(Original::Error(_)) => state == Some("unreadable"),
        Some(_) => matches!(state, Some("invalid" | "unsupported")),
    };
    if !matches_source {
        return Err("sandbox source state differs".into());
    }
    Ok(())
}
