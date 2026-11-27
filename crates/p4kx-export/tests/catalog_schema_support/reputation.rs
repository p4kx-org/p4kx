// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One entity of each faction, reputation and mission kind.

use p4kx_extraction::SourceValue;
use serde_json::json;

use super::source_values as values;

const SCOPE: &str = "61000000-0000-4000-8000-000000000001";
const STANDING: &str = "62000000-0000-4000-8000-000000000001";

pub(super) fn factions() -> serde_json::Value {
    let absent = || values::source(SourceValue::Absent);
    let empty = || values::present(json!([]));
    json!({"availability":"available","rows":[{
        "guid":values::present(json!("60000000-0000-4000-8000-000000000001")),
        "recordName":values::present(json!("FactionReputation.FactionReputation_Acme")),
        "className":values::present(json!("FactionReputation_Acme")),
        "recordPath":values::present(json!("libs/foundry/records/factions/factionreputation/factionreputation_acme")),
        "recordIndex":values::present(json!(3)),
        "sourceType":values::present(json!("FactionReputation")),
        "fileName":values::present(json!("libs/foundry/records/factions/factionreputation/factionreputation_acme.xml")),
        "metadataTag":absent(),
        "name":absent(),
        "displayName":absent(),
        "description":absent(),
        "background":absent(),
        "geid":values::present(json!("18446744073709551610")),
        "logo":absent(),
        "missionBoardLogo":absent(),
        "guildBanner":absent(),
        "hideInDelpihApp":absent(),
        "isNpc":values::present(json!(false)),
        "reputationContextPropertiesUi":absent(),
        "allies":empty(),
        "enemies":empty(),
        "hostilityParams":absent(),
        "alliedParams":absent(),
        "perkReputationRewardList":empty(),
        "propertiesBb":empty(),
        "authoredFields":{"roots":[],"values":[]}
    }]})
}

pub(super) fn scopes() -> serde_json::Value {
    json!({"availability":"available","rows":[{
        "guid":values::present(json!(SCOPE)),
        "recordName":values::present(json!("FactionReputationScope")),
        "className":values::present(json!("FactionReputationScope")),
        "recordPath":values::present(json!("libs/foundry/records/reputation/scopes/factionreputationscope")),
        "recordIndex":values::present(json!(1)),
        "sourceType":values::present(json!("SReputationScopeParams")),
        "fileName":values::present(json!("libs/foundry/records/reputation/scopes/factionreputationscope.xml")),
        "metadataTag":values::source(SourceValue::Absent),
        "scopeName":values::present(json!("FactionReputation")),
        "displayName":values::source(SourceValue::Absent),
        "description":values::source(SourceValue::Absent),
        "icon":values::present(json!("ui/fixture/reputation_scope.dds")),
        "standingMap":values::present(json!({
            "sourceType":values::present(json!("SReputationStandingMapParams")),
            "reputationCeiling":values::present(json!({"type":"int64","value":"9007199254740993"})),
            "initialReputation":values::present(json!({"type":"int64","value":"0"})),
            "standings":values::present(json!([])),
            "authoredFields":{"roots":[],"values":[]}
        })),
        "authoredFields":{"roots":[],"values":[]}
    }]})
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
    let property = values::present(json!({
        "sourceType":values::present(json!("SReputationContextBBPropertyParams")),
        "name":values::present(json!("fixtureLabel")),
        "dynamicProperty":values::present(json!({
            "sourceType":values::present(json!("SBBDynamicPropertyLocString")),
            "kind":"localized",
            "value":values::source(SourceValue::Null),
            "authoredFields":{"roots":[],"values":[]}
        })),
        "authoredFields":{"roots":[],"values":[]}
    }));
    json!({"availability":"available","rows":[{
        "guid":values::present(json!("63000000-0000-4000-8000-000000000001")),
        "recordName":values::present(json!("SReputationContextUI.Transport")),
        "className":values::present(json!("Transport")),
        "recordPath":values::present(json!("libs/foundry/records/reputation/contexts/transport")),
        "recordIndex":values::present(json!(2)),
        "sourceType":values::present(json!("SReputationContextUI")),
        "fileName":values::present(json!("libs/foundry/records/reputation/contexts/transport.xml")),
        "metadataTag":values::source(SourceValue::Absent),
        "sortOrderScope":values::present(json!("FixtureSortOrder")),
        "primaryScopeContext":values::present(json!({
            "sourceType":values::present(json!("SReputationScopeContextUI")),
            "scope":values::source(SourceValue::Null),
            "propertiesBb":values::present(json!([property])),
            "authoredFields":{"roots":[],"values":[]}
        })),
        "scopeContextList":values::present(json!([])),
        "authoredFields":{"roots":[],"values":[]}
    }]})
}

pub(super) fn mission_types() -> serde_json::Value {
    json!({"availability": "available", "rows": [{
        "guid": values::present(json!("50000000-0000-4000-8000-000000000001")),
        "recordName": values::present(json!("MissionType.missiontype.job")),
        "className": values::present(json!("missiontype.job")),
        "recordPath": values::present(json!("libs/foundry/records/missiontype/missiontype.job")),
        "recordIndex": values::present(json!(0)),
        "sourceType": values::present(json!("MissionType")),
        "fileName": values::present(json!("libs/foundry/records/missiontype/missiontype.job.xml")),
        "metadataTag": values::source(SourceValue::Absent),
        "typeName": values::present(json!("text:9:@type_job"))
    }]})
}

pub(super) fn mission_type_text() -> serde_json::Value {
    json!({
        "id": values::present(json!("text:9:@type_job")),
        "key": values::present(json!("@type_job")),
        "translations": values::present(json!({
            "en": {
                "sourceText": values::present(json!("Job")),
                "displayText": values::present(json!("Job"))
            }
        }))
    })
}

pub(super) fn localities() -> serde_json::Value {
    json!({"availability": "available", "rows": [{
        "guid": values::present(json!("64000000-0000-4000-8000-000000000001")),
        "recordName": values::present(json!("MissionLocality.pyro")),
        "className": values::present(json!("pyro")),
        "recordPath": values::present(json!("libs/foundry/records/missiondata/pu_missionlocality/pyro")),
        "recordIndex": values::present(json!(1)),
        "sourceType": values::present(json!("MissionLocality")),
        "fileName": values::present(json!("libs/foundry/records/missiondata/pu_missionlocality/pyro.xml")),
        "metadataTag": values::source(SourceValue::Absent),
        "availableLocations": values::present(json!([values::present(json!({
            "guid": values::present(json!("a2000000-0000-4000-8000-000000000001")),
            "targetRecordIndex": values::present(json!(2)),
            "recordPath": values::present(json!("libs/foundry/records/starmap/pu/fixture")),
            "targetType": values::present(json!("StarMapObject")),
            "status": "present"
        }))])),
        "authoredFields": {"roots": [], "values": []}
    }]})
}

pub(super) fn scenarios() -> serde_json::Value {
    let present = values::present;
    let link = |guid, index, path, source_type| {
        present(json!({
            "guid": present(json!(guid)),
            "targetRecordIndex": present(json!(index)),
            "recordPath": present(json!(path)),
            "targetType": present(json!(source_type)),
            "status": "present"
        }))
    };
    let path = "libs/foundry/records/contracts/contractscenarios/ors_scenarioprogress";
    let reward = present(json!({
        "minPoints": present(json!({"type":"int32","value":10800})),
        "badgeToAward": present(json!("ORS_Pistol")),
        "globalPointsMultiplier": present(json!({"type":"f32","rawBits":"3f800000","finite":1.0})),
        "blueprintPools": present(json!([]))
    }));
    let progression = present(json!({
        "minMultiplierThreshold": present(json!({"type":"f32","rawBits":"3f800000","finite":1.0})),
        "completionType": values::source(SourceValue::Absent),
        "periodMissionScenario": values::source(SourceValue::Null),
        "tiers": present(json!([reward])),
        "authoredFields": {"roots":[],"values":[]}
    }));
    let faction = present(json!({
        "faction": link("66000000-0000-4000-8000-000000000001", 1, "factions/faction", "Faction"),
        "factionName": present(json!("faction")),
        "factionReputation": link("60000000-0000-4000-8000-000000000001", 2, "factions/reputation", "FactionReputation"),
        "pointsAsPercent": present(json!(false)),
        "progressions": present(json!([progression]))
    }));
    json!([{
        "guid": present(json!("65000000-0000-4000-8000-000000000001")),
        "recordName": present(json!("ScenarioProgress.ORS_ScenarioProgress")),
        "className": present(json!("ORS_ScenarioProgress")),
        "recordPath": present(json!(path)),
        "recordIndex": present(json!(0)),
        "sourceType": present(json!("ScenarioProgress")),
        "fileName": present(json!(format!("{path}.xml"))),
        "metadataTag": values::source(SourceValue::Absent),
        "factionRewardTiers": present(json!([faction]))
    }])
}
