// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One entity of each faction, reputation and mission kind.

use p4kx_extraction::SourceValue;
use serde_json::json;

use super::source_values as values;

const SCOPE: &str = "61000000-0000-4000-8000-000000000001";
const STANDING: &str = "62000000-0000-4000-8000-000000000001";

pub(super) fn factions() -> serde_json::Value {
    json!([{
        "guid": "60000000-0000-4000-8000-000000000001",
        "className": "FactionReputation_Acme",
        "recordPath": "libs/foundry/records/factions/factionreputation/factionreputation_acme",
        "displayName": {"key": "@faction_acme", "en": "Acme Emporium", "de": null},
        "isNpc": false,
        "lawful": true,
        "contextGuid": "63000000-0000-4000-8000-000000000001",
        "allyGuids": [],
        "enemyGuids": ["60000000-0000-4000-8000-000000000002"],
        "hostility": {"scopeGuid": SCOPE, "standingGuid": STANDING},
        "allied": null
    }])
}

pub(super) fn scopes() -> serde_json::Value {
    json!([{
        "guid": SCOPE,
        "className": "FactionReputationScope",
        "recordPath": "libs/foundry/records/reputation/scopes/factionreputationscope",
        "scopeName": "FactionReputation",
        "displayName": {"key": "@scope_standing", "en": "Standing", "de": null},
        "reputationCeiling": 95250,
        "initialReputation": 0,
        "standingGuids": [STANDING]
    }])
}

pub(super) fn standings() -> serde_json::Value {
    json!({"availability":"available","rows":[{
        "guid":values::present(json!(STANDING)),
        "recordName":values::present(json!("ReputationStanding_Faction_B0_00")),
        "className":values::present(json!("ReputationStanding_Faction_B0_00")),
        "recordPath":values::present(json!("libs/foundry/records/reputation/standings/faction/band_0/reputationstanding_faction_b0_00")),
        "recordIndex":values::present(json!(0)),
        "sourceType":values::present(json!("SReputationStandingParams")),
        "fileName":values::present(json!("libs/foundry/records/reputation/standings/faction/band_0/reputationstanding_faction_b0_00.xml")),
        "metadataTag":values::source(SourceValue::Absent),
        "name":values::source(SourceValue::Absent),
        "description":values::source(SourceValue::Absent),
        "displayName":values::source(SourceValue::Absent),
        "perkDescription":values::source(SourceValue::Absent),
        "icon":values::source(SourceValue::Absent),
        "missionBoardIcon":values::source(SourceValue::Absent),
        "minReputation":values::present(json!({"type":"int64","value":"0"})),
        "driftReputation":values::present(json!({"type":"int64","value":"-5"})),
        "driftTimeHours":values::present(json!({"rawBits":"41c40000","finite":24.5})),
        "gated":values::present(json!(false)),
        "additionalFields":{"roots":[],"values":[]}
    }]})
}

pub(super) fn contexts() -> serde_json::Value {
    json!([{
        "guid": "63000000-0000-4000-8000-000000000001",
        "className": "ReputationContext_Transport",
        "recordPath": "libs/foundry/records/reputation/contexts/reputationcontext_transport",
        "primaryScopeGuid": SCOPE,
        "scopeGuids": [SCOPE]
    }])
}

pub(super) fn mission_types() -> serde_json::Value {
    json!([{
        "guid": "50000000-0000-4000-8000-000000000001",
        "className": "missiontype.job",
        "recordPath": "libs/foundry/records/missiontype/missiontype.job",
        "typeName": {"key": "@type_job", "en": "Job", "de": null}
    }])
}

pub(super) fn localities() -> serde_json::Value {
    json!([{
        "guid": "64000000-0000-4000-8000-000000000001",
        "className": "pyro",
        "recordPath": "libs/foundry/records/missiondata/pu_missionlocality/pyro",
        "starmapObjectGuids": ["a2000000-0000-4000-8000-000000000001"]
    }])
}

pub(super) fn scenarios() -> serde_json::Value {
    json!([{
        "guid": "65000000-0000-4000-8000-000000000001",
        "className": "ORS_ScenarioProgress",
        "recordPath": "libs/foundry/records/contracts/contractscenarios/ors_scenarioprogress",
        "factionRewardTiers": [{
            "factionGuid": "66000000-0000-4000-8000-000000000001",
            "factionReputationGuid": "60000000-0000-4000-8000-000000000001",
            "factionName": "faction_lawful_crusaderindustries",
            "pointsAsPercent": false,
            "progressions": [{
                "minMultiplierThreshold": 1.0,
                "completionType": "CompletionType_GlobalProgression",
                "periodScenarioGuid": null,
                "tiers": [{
                    "minPoints": 10800,
                    "badgeToAward": "ORS_Pistol",
                    "globalPointsMultiplier": 1.0,
                    "blueprintPoolGuids": ["e4000000-0000-4000-8000-000000000001"]
                }]
            }]
        }]
    }])
}
