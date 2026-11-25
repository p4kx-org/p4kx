// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::faction_identities::{
    FactionIdentityDefinition, FactionIdentitySnapshot,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, Filter, build_catalog};
use p4kx_extraction::Guid;
use p4kx_testkit::TestType;
use serde_json::{Value, json};

use crate::canonical_header::json::assert_canonical;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};
use crate::sdk_full_proof as full_proof;

#[path = "catalog_faction_identity/fixture.rs"]
mod fixture;
use fixture::{database, export, identity};

const FACTION: u128 = 0x1085_0001;
const TAG: u128 = 0x1085_0002;
const SECURITY_FIRST: u128 = 0x1085_0003;
const SECURITY_SECOND: u128 = 0x1085_0004;
const FIRST: u128 = 0x1085_0005;
const SECOND: u128 = 0x1085_0006;
const MISSING: u128 = 0x1085_00ff;
const NAME: &str = "FixtureFactionIdentityName";
const DESCRIPTION: &str = "FixtureFactionIdentityDescription";

#[test]
fn faction_identities_keep_their_own_links_weights_and_protected_text() -> TestResult {
    let catalog = export(None, TestType::Single, false)?;
    let identities = at(&catalog, "/definitions/factionIdentities/rows")?
        .as_array()
        .ok_or("physical identity rows")?;
    let [first, second] = identities.as_slice() else {
        return Err("both physical identities must be retained".into());
    };
    assert_eq!(
        at(first, "/guid/value")?,
        &json!(guid(FIRST)),
        "first identity"
    );
    assert_eq!(
        at(second, "/guid/value")?,
        &json!(guid(SECOND)),
        "second identity"
    );
    verify_identity_links(&catalog, first, second)?;
    verify_weights_and_theme_tags(first, second)?;
    let archetype = at(&catalog, "/definitions/npcArchetypes/rows/0")?;
    assert_eq!(
        at(
            archetype,
            "/identity/value/0/value/identity/value/guid/value"
        )?,
        &json!(guid(FIRST)),
        "existing NPC branch joins by physical identity"
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    if let Err(error) = validator.validate(&catalog) {
        return Err(format!("{}: {error}", error.instance_path()).into());
    }
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

fn verify_identity_links(catalog: &Value, first: &Value, second: &Value) -> TestResult {
    for (row, security) in [(first, SECURITY_FIRST), (second, SECURITY_SECOND)] {
        assert_eq!(
            at(row, "/factionRef/value/guid/value")?,
            &json!(guid(FACTION)),
            "both owners retain the same Faction"
        );
        for (field, key) in [("name", NAME), ("description", DESCRIPTION)] {
            let cell = at(row, &format!("/{field}"))?;
            assert_eq!(at(cell, "/containsEncryptedSource")?, true, "{field}");
            let text = common::text_row(catalog, "gameTexts", at(cell, "/value")?)?;
            assert_eq!(at(text, "/key/value")?, key, "{field}");
        }
        assert_eq!(
            at(row, "/securityNetworkRef/value/guid/value")?,
            &json!(guid(security)),
            "each owner keeps its distinct security network"
        );
    }
    Ok(())
}

fn verify_weights_and_theme_tags(first: &Value, second: &Value) -> TestResult {
    for (index, preset, bits) in [
        (0, "FixtureAlpha", "80000000"),
        (1, "FixtureFuture", "3fa00000"),
    ] {
        let weight = at(
            first,
            &format!("/encounterPresetWeights/value/{index}/value"),
        )?;
        assert_eq!(
            at(weight, "/encounterPreset/value")?,
            preset,
            "preserve authored preset enum name and order"
        );
        assert_eq!(
            at(weight, "/weight/value/rawBits")?,
            bits,
            "exact authored Single"
        );
        assert_eq!(
            at(weight, "/weight/containsEncryptedSource")?,
            true,
            "weight is individually sourced"
        );
    }
    assert_eq!(
        at(first, "/lootPoolThemeTags/value/0/value/guid/value")?,
        &json!(guid(TAG)),
        "original theme Tag"
    );
    assert_eq!(
        at(first, "/lootPoolThemeTags/value/1/state")?,
        "null",
        "null list position"
    );
    assert_eq!(
        at(first, "/lootPoolThemeTags/value/2/value/status")?,
        "dangling",
        "dangling tag remains in position"
    );
    assert_eq!(
        at(first, "/lootPoolThemeTags/value/2/value/guid/value")?,
        &json!(guid(MISSING)),
        "dangling target GUID"
    );
    assert_eq!(
        at(second, "/lootPoolThemeTags/value")?,
        &json!([]),
        "authored empty tag array"
    );
    assert_eq!(
        at(second, "/encounterPresetWeights/value")?,
        &json!([]),
        "authored empty weight array"
    );
    Ok(())
}

#[test]
fn filtered_faction_targets_leave_exact_identity_references() -> TestResult {
    let filter = Filter::new("*/faction_identity_fixture/identities/*");
    let catalog = export(Some(&filter), TestType::Single, false)?;
    assert_eq!(
        at(&catalog, "/definitions/factions/rows")?,
        &json!([]),
        "filter omits the Faction row"
    );
    let identity = at(&catalog, "/definitions/factionIdentities/rows/0")?;
    assert_eq!(
        at(identity, "/factionRef/value/guid/value")?,
        &json!(guid(FACTION)),
        "identity retains the exact source target"
    );
    let rows = at(&catalog, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or("unresolved catalog links")?;
    assert!(
        rows.iter().any(|row| {
            row["status"] == "excludedByFilter" && row["targetGuid"]["value"] == guid(FACTION)
        }),
        "filtered target gets explicit catalog status"
    );

    let filter = Filter::new("*/faction_identity_fixture/archetypes/*");
    let catalog = export(Some(&filter), TestType::Single, false)?;
    assert_eq!(
        at(&catalog, "/definitions/factionIdentities/rows")?,
        &json!([]),
        "identity row can be filtered independently"
    );
    let branch = at(
        &catalog,
        "/definitions/npcArchetypes/rows/0/identity/value/0/value/identity",
    )?;
    assert_eq!(
        at(branch, "/value/guid/value")?,
        &json!(guid(FIRST)),
        "NPC source link survives its filtered target"
    );
    Ok(())
}

#[test]
fn malformed_encounter_weight_retains_its_original_member_and_health() -> TestResult {
    let catalog = export(None, TestType::String, true)?;
    let weight = at(
        &catalog,
        "/definitions/factionIdentities/rows/0/encounterPresetWeights/value/0/value",
    )?;
    assert_eq!(
        at(weight, "/weight/state")?,
        "invalid",
        "mistyped weight is not converted to a number"
    );
    let values = at(weight, "/authoredFields/values")?
        .as_array()
        .ok_or("retained original fields")?;
    assert!(
        !values.is_empty(),
        "original malformed value stays in the source arena"
    );
    assert_eq!(
        at(&catalog, "/exportHealth/status")?,
        "incomplete",
        "invalid authored weight is visible to export health"
    );
    Ok(())
}

#[test]
fn faction_identity_sdk_selects_canonical_rows_and_physical_duplicates() -> TestResult {
    let database = database(TestType::Single).record(identity(
        SECOND,
        "identities/second_duplicate",
        SECURITY_FIRST,
        "resource_name",
    ));
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    assert!(
        matches!(
            build_catalog(&archive, common::options().with_full_provenance()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "a complete catalog refuses ambiguous physical owners"
    );
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog = build_catalog(&archive, options.with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(catalog.json())?;
    let definitions = at(&catalog, "/definitions/factionIdentities/rows")?;
    let session = full_proof::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let (duplicate_guid, index) =
        parsed.with_faction_identities(CatalogRecordSelection::All, None, |snapshot| {
            inspect_all(&snapshot, definitions, &catalog)
        })??;
    let duplicate_indices = parsed.with_faction_identities(
        CatalogRecordSelection::Guid(duplicate_guid),
        None,
        |s| physical_indices(&s),
    )??;
    let [first, second] = duplicate_indices.as_slice() else {
        return Err("both physical owners of the requested GUID".into());
    };
    assert!(*first < *second, "tied GUID owners retain physical order");
    assert_eq!(
        *second, index,
        "the query retains the last physical duplicate"
    );
    parsed.with_faction_identities(CatalogRecordSelection::RecordIndex(index), None, |s| {
        assert_eq!(
            physical_indices(&s)?,
            [index],
            "physical selection narrows the duplicate GUID"
        );
        Ok::<(), Box<dyn std::error::Error>>(())
    })??;
    parsed.with_faction_identities(CatalogRecordSelection::RecordIndex(0), None, |s| {
        assert!(
            s.rows.is_empty(),
            "a different Record family is not admitted"
        );
        assert!(
            s.available_languages.is_empty(),
            "empty selection reads no locale"
        );
    })?;
    Ok(())
}

fn inspect_all(
    snapshot: &FactionIdentitySnapshot<'_>,
    definitions: &Value,
    catalog: &Value,
) -> TestResult<(Guid, u32)> {
    assert!(
        matches!(snapshot.availability, DatasetAvailability::Available),
        "the FactionIdentity family is declared"
    );
    assert!(
        snapshot.coverage.incomplete_fields.is_empty(),
        "typed fields are covered"
    );
    let [_, _, duplicate] = snapshot.rows.as_slice() else {
        return Err("all three physical rows".into());
    };
    assert_canonical(&snapshot.rows, definitions, "canonical faction identities")?;
    let typed = duplicate.name();
    assert_eq!(
        typed.origin().contains_encrypted_source(),
        Some(true),
        "the encrypted original locale marker survives the typed API"
    );
    let text = snapshot
        .game_text(typed.value().ok_or("authored protected name link")?)
        .ok_or("protected name row")?;
    let key = text.id().value().ok_or("protected text identity")?;
    let original = common::text_row(catalog, "gameTexts", &json!(key))?;
    assert_canonical(text, original, "protected name and translations")?;
    assert_eq!(
        text.translations()
            .value()
            .map(std::collections::BTreeMap::len),
        Some(2),
        "both available locales contribute to the selected protected text"
    );
    let guid = duplicate
        .source()
        .guid()
        .value()
        .copied()
        .ok_or("duplicate GUID")?;
    Ok((guid, physical_index(duplicate)?))
}

fn physical_indices(snapshot: &FactionIdentitySnapshot<'_>) -> TestResult<Vec<u32>> {
    snapshot.rows.iter().map(physical_index).collect()
}

fn physical_index(row: &FactionIdentityDefinition<'_>) -> TestResult<u32> {
    row.source()
        .record_index()
        .value()
        .copied()
        .ok_or_else(|| "physical Record index".into())
}
