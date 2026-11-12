// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, Filter, Kind, build_catalog};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as archive, DcbOrigin, TestResult, at, guid, text_row};

#[path = "catalog_faction_fixture.rs"]
mod fixture;

const ROWS: &str = "/definitions/factions/rows";

fn build(
    database: &DataForgeBuilder,
    origin: DcbOrigin,
    filter: Option<&Filter>,
) -> TestResult<(BuiltCatalog, Value)> {
    let (_folder, archive) = archive::open(database, origin)?;
    let mut options = archive::options();
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn rows(catalog: &Value) -> TestResult<&[Value]> {
    Ok(at(catalog, ROWS)?.as_array().ok_or("Faction rows")?)
}

#[test]
fn factions_export_exact_legal_behavior_ordered_links_and_unprojected_fields() -> TestResult {
    let database = fixture::complete();
    let (_folder, archive) = archive::open(&database, DcbOrigin::Plain)?;
    let options = archive::options();
    let built = build_catalog(&archive, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    let rows = rows(&catalog)?;
    assert_eq!(
        rows.len(),
        3,
        "type selection retains authored paths without a name rule"
    );
    let [security, civil, outside] = rows else {
        return Err("three physical Faction rows".into());
    };
    assert_eq!(
        at(security, "/guid/value")?,
        &json!(guid(fixture::SECURITY))
    );
    assert_eq!(at(civil, "/guid/value")?, &json!(guid(fixture::CIVIL)));
    assert_eq!(at(outside, "/guid/value")?, &json!(guid(fixture::OUTSIDE)));
    assert_eq!(
        at(security, "/defaultReaction/value")?,
        &json!(fixture::UNKNOWN_REACTION),
        "unrecognised authored vocabulary remains a label"
    );
    assert_eq!(
        at(security, "/factionType/value")?,
        &json!("FixtureSecurity")
    );
    for (name, expected) in fixture::FLAGS.into_iter().zip([true, false, true, false]) {
        let cell = at(security, &format!("/{name}"))?;
        assert_eq!(at(cell, "/state")?, "present", "{name}");
        assert_eq!(at(cell, "/value")?, &json!(expected), "{name}");
        assert_eq!(at(cell, "/containsEncryptedSource")?, false, "{name}");
    }
    assert_eq!(at(civil, "/ableToArrest/value")?, false);
    assert_eq!(at(civil, "/noLegalRights/value")?, true);
    assert_links(security, civil)?;
    assert_friendly_fire(security, civil)?;
    assert_raw(security)?;
    assert_eq!(at(&catalog, "/completeness/factions/status")?, "complete");
    let summary = built
        .summary()
        .kind(Kind::Factions)
        .ok_or("Faction summary")?;
    assert_eq!(summary.count, rows.len());
    assert!(summary.complete, "unfiltered Faction selection");
    assert_eq!(
        at(&catalog, "/factionReputations/0/guid")?,
        &json!(guid(fixture::REPUTATION))
    );
    assert!(
        built.json() == build_catalog(&archive, &options)?.json(),
        "the same source and export timestamp produce identical bytes"
    );
    Ok(())
}

fn assert_links(security: &Value, civil: &Value) -> TestResult {
    for (index, id) in [fixture::CIVIL, fixture::MISSING, fixture::CIVIL]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            at(
                security,
                &format!("/alliedFactions/value/{index}/value/guid/value")
            )?,
            &json!(guid(id)),
            "authored ally at occurrence {index} stays in order"
        );
    }
    assert_eq!(
        at(security, "/alliedFactions/value/1/value/status")?,
        "dangling",
        "a missing ally keeps its authored position and state"
    );
    assert_eq!(
        at(security, "/enemyFactions/value/0/value/guid/value")?,
        &json!(guid(fixture::OUTSIDE)),
        "the target has a physical Faction outside the usual folder"
    );
    assert_eq!(
        at(security, "/factionReputationRef/value/guid/value")?,
        &json!(guid(fixture::REPUTATION)),
        "the linked reputation identity stays distinct"
    );
    assert_eq!(
        at(civil, "/factionReputationRef/value/guid/value")?,
        &json!(guid(fixture::MISSING)),
        "the dangling reputation retains its stored GUID"
    );
    assert_eq!(
        at(civil, "/factionReputationRef/value/status")?,
        "dangling",
        "the link records its physical target state"
    );
    Ok(())
}

fn assert_friendly_fire(security: &Value, civil: &Value) -> TestResult {
    for (index, (reaction, allow)) in [(fixture::UNKNOWN_REACTION, false), ("FixturePassive", true)]
        .into_iter()
        .enumerate()
    {
        let row = at(
            security,
            &format!("/friendlyFireBehaviorOverrides/value/{index}/value"),
        )?;
        assert_eq!(
            at(row, "/reactionType/value")?,
            reaction,
            "original override reaction at {index}"
        );
        assert_eq!(
            at(row, "/shouldAllowFriendlyFire/value")?,
            allow,
            "original override Bool at {index}"
        );
    }
    assert_eq!(
        at(civil, "/friendlyFireBehaviorOverrides/value")?,
        &json!([]),
        "a declared empty array stays present and empty"
    );
    Ok(())
}

fn assert_raw(security: &Value) -> TestResult {
    let raw = at(security, "/authoredFields")?;
    let roots = at(raw, "/roots")?.as_array().ok_or("Faction raw roots")?;
    let values = at(raw, "/values")?.as_array().ok_or("Faction raw values")?;
    let names: Vec<_> = roots
        .iter()
        .map(|root| at(root, "/name/value"))
        .collect::<Result<_, _>>()?;
    assert_eq!(
        names,
        [
            &json!("fixtureAdditionalLegalFlag"),
            &json!("fixtureUnclassifiedNumber")
        ],
        "the source arena contains the exact remaining declaration order"
    );
    assert_eq!(
        values.len(),
        roots.len(),
        "unprojected source occurrences appear once"
    );
    Ok(())
}

#[test]
fn faction_text_domains_and_inherited_absence_keep_exact_origins() -> TestResult {
    let (_, catalog) = build(&fixture::complete(), DcbOrigin::Encrypted, None)?;
    let security = rows(&catalog)?.first().ok_or("security Faction")?;
    for (field, table) in [("name", "publicNames"), ("description", "gameTexts")] {
        let cell = at(security, &format!("/{field}"))?;
        assert_eq!(at(cell, "/containsEncryptedSource")?, true, "{field}");
        assert_eq!(at(cell, "/value/type")?, "localized", "{field}");
        let reference = at(cell, "/value/reference")?;
        let row = text_row(&catalog, table, reference)?;
        assert_eq!(at(row, "/key/value")?, fixture::SECURITY_NAME, "{field}");
        assert_eq!(
            at(row, "/translations/value/de/displayText/value")?,
            "Gemeinsamer Text",
            "{field}"
        );
    }
    assert_eq!(at(security, "/ableToArrest/containsEncryptedSource")?, true);
    assert_eq!(
        at(
            security,
            "/friendlyFireBehaviorOverrides/value/1/value/reactionType/containsEncryptedSource"
        )?,
        true
    );
    crate::commodity_v3_source_inventory::reference_closure(&catalog)?;

    let (_, legacy) = build(&fixture::legacy(), DcbOrigin::Plain, None)?;
    let row = rows(&legacy)?.first().ok_or("legacy Faction")?;
    assert_eq!(
        at(row, "/name/value")?,
        &json!({"type":"literal","text":"Fixture short name"})
    );
    assert_eq!(
        at(row, "/description/value")?,
        &json!({"type":"literal","text":fixture::DESCRIPTION})
    );
    assert_eq!(at(row, "/factionReputationRef/state")?, "null");
    for name in fixture::FLAGS {
        assert_eq!(at(row, &format!("/{name}/state"))?, "absent", "{name}");
        assert!(at(row, &format!("/{name}/value"))?.is_null(), "{name}");
    }
    Ok(())
}

#[test]
fn scenario_keeps_original_faction_guid_when_its_target_is_filtered() -> TestResult {
    let database = fixture::complete();
    let (_, full) = build(&database, DcbOrigin::Plain, None)?;
    let tier = "/contractScenarios/0/factionRewardTiers/0";
    let unfiltered = at(&full, tier)?;
    assert_eq!(
        at(unfiltered, "/factionGuid")?,
        &json!(guid(fixture::SECURITY))
    );
    assert_eq!(
        at(unfiltered, "/factionReputationGuid")?,
        &json!(guid(fixture::REPUTATION))
    );
    assert_eq!(
        at(rows(&full)?.first().ok_or("joined Faction")?, "/guid/value")?,
        at(unfiltered, "/factionGuid")?
    );
    let filter = Filter::new("contractscenarios");
    let (_, filtered) = build(&database, DcbOrigin::Plain, Some(&filter))?;
    let no_factions: &[Value] = &[];
    assert_eq!(
        rows(&filtered)?,
        no_factions,
        "the filter excludes Faction rows"
    );
    assert_eq!(
        at(&filtered, "/definitions/factions/availability")?,
        "available"
    );
    assert_eq!(at(&filtered, "/completeness/factions/status")?, "partial");
    assert_eq!(at(&filtered, tier)?, unfiltered);
    for catalog in [&full, &filtered] {
        let missing = at(catalog, "/contractScenarios/0/factionRewardTiers/1")?;
        assert_eq!(at(missing, "/factionGuid")?, &json!(guid(fixture::MISSING)));
        assert!(at(missing, "/factionReputationGuid")?.is_null());
    }
    Ok(())
}

#[test]
fn unclassified_nonfinite_faction_number_has_an_emitted_lossless_receipt() -> TestResult {
    let (_, catalog) = build(&fixture::unclassified_nonfinite(), DcbOrigin::Plain, None)?;
    let row = rows(&catalog)?
        .first()
        .ok_or("Faction with an unclassified number")?;
    let nodes = at(row, "/authoredFields/values")?
        .as_array()
        .ok_or("raw Faction values")?;
    let number = nodes.last().ok_or("unclassified number node")?;
    assert_eq!(
        at(number, "/value/value/value/rawBits")?,
        "7fc00666",
        "exact original NaN bits"
    );
    assert_eq!(at(&catalog, "/exportHealth/status")?, "complete");
    Ok(())
}

#[test]
fn faction_wire_contract_requires_the_nullable_cells_and_protects_descriptions() -> TestResult {
    let (_, catalog) = build(&fixture::complete(), DcbOrigin::Plain, None)?;
    let schema = p4kx_export::catalog_schema();
    assert_eq!(
        at(
            &schema,
            "/$defs/FactionDefinition/properties/description/x-gameText"
        )?,
        true
    );
    assert!(
        at(&schema, "/$defs/FactionDefinition/properties/name")?
            .get("x-gameText")
            .is_none()
    );
    let validator = jsonschema::validator_for(&schema)?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    let mut missing = catalog;
    let row = missing
        .pointer_mut("/definitions/factions/rows/0")
        .and_then(Value::as_object_mut)
        .ok_or("sourced Faction row")?;
    row.remove("policesLawfulTrespass")
        .ok_or("required nullable flag")?;
    assert!(
        !validator.is_valid(&missing),
        "all direct fields are required even when absent"
    );
    Ok(())
}

#[test]
fn an_inherited_flag_uses_the_original_encrypted_physical_field() -> TestResult {
    let (_, catalog) = build(&fixture::complete(), DcbOrigin::Encrypted, None)?;
    let security = rows(&catalog)?.first().ok_or("original security row")?;
    let index = at(security, "/recordIndex/value")?
        .as_u64()
        .ok_or("physical index")?
        .to_string();
    let carrier = at(security, "/ableToArrest")?;
    let references = at(carrier, "/sourceUsesRefs")?
        .as_array()
        .ok_or("immediate uses")?;
    let uses = at(&catalog, "/provenance/uses")?
        .as_array()
        .ok_or("provenance uses")?;
    assert!(
        uses.iter().any(|usage| {
            usage.get("id").is_some_and(|id| references.contains(id))
                && usage.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
                && usage.pointer("/location/indices") == Some(&json!([index, "0"]))
                && usage.pointer("/location/field") == Some(&json!("$value"))
                && usage.get("role") == Some(&json!("payload"))
                && usage.get("effectiveEncryption") == Some(&json!("encrypted"))
        }),
        "the exact inherited first payload occurrence is encrypted"
    );
    Ok(())
}
