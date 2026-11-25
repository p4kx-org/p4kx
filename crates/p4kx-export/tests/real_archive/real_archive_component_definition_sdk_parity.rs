// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::env;

use p4kx_export::catalog_queries::equipment::component_definitions::{
    ComponentDefinitionSnapshot, ConsumableSubtype,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability, GameTextRef,
    ParsedCatalogQueries, PublicNameRef,
};
use p4kx_extraction::Archive;
use serde::Serialize;

use crate::real_archive_catalog_table_parity::{
    DefinitionOwner, StreamedDefinitionDigests, TestResult, with_bound_measurement_from_producer,
};
use crate::real_archive_scoped_equipment_sdk::equipment_input;
use crate::real_archive_verification::print_line;

const PRODUCER: &str = "P4KX_COMPONENT_DEFINITION_CATALOG_PRODUCER_REVISION";
const COLD_DATASET: &str = "P4KX_COMPONENT_DEFINITION_SDK_COLD_DATASET";
const COUNT: usize = 5;

#[derive(Clone, Copy)]
enum Family {
    DamageResistanceMacros,
    MiningLaserGlobals,
    TurretHealthModifiers,
    ConsumableTypes,
    ConsumableSubtypes,
}

const FAMILIES: [Family; COUNT] = [
    Family::DamageResistanceMacros,
    Family::MiningLaserGlobals,
    Family::TurretHealthModifiers,
    Family::ConsumableTypes,
    Family::ConsumableSubtypes,
];

impl Family {
    const fn path(self) -> &'static str {
        match self {
            Self::DamageResistanceMacros => "/definitions/damageResistanceMacros",
            Self::MiningLaserGlobals => "/definitions/miningLaserGlobals",
            Self::TurretHealthModifiers => "/definitions/turretHealthModifiers",
            Self::ConsumableTypes => "/definitions/consumableTypes",
            Self::ConsumableSubtypes => "/definitions/consumableSubtypes",
        }
    }

    const fn owner(self) -> DefinitionOwner {
        match self {
            Self::DamageResistanceMacros
            | Self::MiningLaserGlobals
            | Self::TurretHealthModifiers => DefinitionOwner::NestedRecord,
            Self::ConsumableTypes | Self::ConsumableSubtypes => DefinitionOwner::Record,
        }
    }

    const fn has_text_scope(self) -> bool {
        matches!(self, Self::ConsumableSubtypes)
    }
}

struct Retention<'proof, 'scratch> {
    table: usize,
    proof: &'proof mut StreamedDefinitionDigests<'scratch, COUNT>,
}

#[test]
#[ignore = "installed Archive: configure the expected Build and one exact component definition"]
fn installed_component_definition_cold_query() -> TestResult {
    let path = env::var(COLD_DATASET)?;
    let family = FAMILIES
        .into_iter()
        .find(|family| family.path() == path)
        .ok_or("configure one exact component definition dataset path")?;
    let languages = if family.has_text_scope() {
        CatalogLanguageSelection::AllAvailable
    } else {
        CatalogLanguageSelection::Selected(&[])
    };
    equipment_input::with_archive(|archive| {
        let session = equipment_input::full_source_session(archive, languages)?;
        read_family(archive, &session.parsed(None)?, family, None)
    })
}

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_component_definitions_match_one_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let scratch = tempfile::tempdir()?;
        let mut proof = StreamedDefinitionDigests::new(scratch.path(), FAMILIES.map(Family::owner));
        {
            let session = equipment_input::full_source_session(
                archive,
                CatalogLanguageSelection::AllAvailable,
            )?;
            let parsed = session.parsed(None)?;
            for (table, family) in FAMILIES.into_iter().enumerate() {
                read_family(
                    archive,
                    &parsed,
                    family,
                    Some(Retention {
                        table,
                        proof: &mut proof,
                    }),
                )?;
            }
        }
        let compared = proof.compare(measurement, FAMILIES.map(Family::path))?;
        print_line(&format!(
            "complete Component Definitions SDK parity: rows={:?} sourceUses={} physicalSources={}",
            compared.rows, compared.source_uses, compared.physical_sources
        ));
        Ok(())
    })
}

fn read_family(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    family: Family,
    retention: Option<Retention<'_, '_>>,
) -> TestResult {
    let all = CatalogRecordSelection::All;
    match family {
        Family::DamageResistanceMacros => {
            parsed.with_damage_resistance_macros(all, None, |sdk| {
                observe(archive, family, &sdk, retention)
            })?
        }
        Family::MiningLaserGlobals => parsed.with_mining_laser_globals(all, None, |sdk| {
            observe(archive, family, &sdk, retention)
        })?,
        Family::TurretHealthModifiers => parsed.with_turret_health_modifiers(all, None, |sdk| {
            observe(archive, family, &sdk, retention)
        })?,
        Family::ConsumableTypes => parsed
            .with_consumable_types(all, None, |sdk| observe(archive, family, &sdk, retention))?,
        Family::ConsumableSubtypes => parsed.with_consumable_subtypes(all, None, |sdk| {
            verify_subtype_text_ids(&sdk)?;
            observe(archive, family, &sdk, retention)
        })?,
    }
}

fn observe<Row: Serialize>(
    archive: &Archive,
    family: Family,
    sdk: &ComponentDefinitionSnapshot<'_, Row>,
    retention: Option<Retention<'_, '_>>,
) -> TestResult {
    let path = family.path();
    let availability = sdk.availability();
    let rows = sdk.rows();
    let coverage = sdk.coverage();
    if sdk.build().is_none()
        || sdk.build() != archive.build()
        || !known_population(availability, rows.len())
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
    {
        return Err(format!("{path}: expected complete row evidence in the matching Build").into());
    }
    require_discovery(path, sdk)?;
    verify_language_scope(family, sdk)?;
    let inventory = sdk.source_inventory();
    let uses = inventory.use_count();
    let sources = inventory.source_count();
    if !rows.is_empty() && (uses == 0 || sources == 0) {
        return Err(format!("{path}: the rows lack original source evidence").into());
    }
    if let Some(retention) = retention {
        retention
            .proof
            .retain(retention.table, availability, rows, inventory)?;
        if family.has_text_scope() {
            retention
                .proof
                .retain_texts(sdk.public_names(), sdk.game_texts(), inventory)?;
        }
    }
    print_line(&format!(
        "Component Definitions SDK family: dataset={path} availability={availability:?} rows={} publicNames={} protectedTexts={} requestedLanguages={} availableLanguages={} inventoryUses={uses} inventorySources={sources} skippedLeftovers={} discovery=complete",
        rows.len(),
        sdk.public_names().len(),
        sdk.game_texts().len(),
        sdk.requested_languages().len(),
        sdk.available_languages().len(),
        coverage.skipped_leftovers,
    ));
    Ok(())
}

fn require_discovery<Row>(path: &str, sdk: &ComponentDefinitionSnapshot<'_, Row>) -> TestResult {
    let coverage = sdk.definition_discovery();
    if !coverage.is_complete() {
        return Err(format!(
            "{path}: definition discovery is incomplete (directoryComplete={}, failedRecords={}, failedEntries={}, incompleteRecords={}, incompleteFields={})",
            coverage.directory_complete(),
            coverage.failed_records().len(),
            coverage.failed_entries().len(),
            coverage.incomplete_records().len(),
            coverage.incomplete_fields(),
        ).into());
    }
    Ok(())
}

fn verify_language_scope<Row>(
    family: Family,
    sdk: &ComponentDefinitionSnapshot<'_, Row>,
) -> TestResult {
    let requested = sdk.requested_languages();
    let available = sdk.available_languages();
    let has_texts = !sdk.public_names().is_empty() || !sdk.game_texts().is_empty();
    let valid = if family.has_text_scope() {
        !requested.is_empty()
            && available
                .iter()
                .all(|language| requested.contains(language))
            && !(sdk.rows().is_empty() && (!available.is_empty() || has_texts))
    } else {
        requested.is_empty() && available.is_empty() && !has_texts
    };
    if !valid {
        return Err(format!("{}: inconsistent scoped language coverage", family.path()).into());
    }
    Ok(())
}

fn verify_subtype_text_ids(
    sdk: &ComponentDefinitionSnapshot<'_, ConsumableSubtype<'_>>,
) -> TestResult {
    let mut names = HashSet::new();
    let mut effects = HashSet::new();
    for row in sdk.rows() {
        if let Some(name) = row.consumable_name().value() {
            names.insert(name.as_str());
        }
        if let Some(slots) = row.effects().value() {
            for slot in slots {
                if let Some(description) = slot
                    .value()
                    .and_then(|effect| effect.effect_description().value())
                {
                    effects.insert(description.as_str());
                }
            }
        }
    }
    let public = sdk
        .public_names()
        .iter()
        .map(|text| text.id().value().map(PublicNameRef::as_str));
    let protected = sdk
        .game_texts()
        .iter()
        .map(|text| text.id().value().map(GameTextRef::as_str));
    if !matches_text_ids(names, public) || !matches_text_ids(effects, protected) {
        return Err("consumable subtype text rows do not match the exact referenced public and protected IDs".into());
    }
    Ok(())
}

fn matches_text_ids<'text>(
    mut expected: HashSet<&'text str>,
    mut actual: impl Iterator<Item = Option<&'text str>>,
) -> bool {
    actual.all(|id| id.is_some_and(|id| expected.remove(id))) && expected.is_empty()
}

const fn known_population(availability: DatasetAvailability, rows: usize) -> bool {
    matches!(
        (availability, rows),
        (DatasetAvailability::Available, _) | (DatasetAvailability::Absent, 0)
    )
}

#[test]
fn component_definition_parity_requires_exact_scoped_text_ids() {
    let expected = || HashSet::from(["first", "second"]);
    assert!(
        matches_text_ids(expected(), [Some("second"), Some("first")].into_iter()),
        "unique referenced text IDs resolve independently of their row order",
    );
    for rows in [
        [Some("first"), Some("first")],
        [Some("first"), Some("unselected")],
        [Some("first"), None],
    ] {
        assert!(
            !matches_text_ids(expected(), rows.into_iter()),
            "duplicate, foreign and unreadable text IDs cannot establish the requested scope",
        );
    }
    assert!(
        !matches_text_ids(expected(), [Some("first")].into_iter()),
        "a missing protected or public text row cannot establish the requested scope",
    );
}

#[test]
fn component_definition_parity_retains_declared_empty_and_absent_flows() {
    assert!(
        known_population(DatasetAvailability::Available, 0),
        "a declared empty definition family remains available",
    );
    assert!(
        known_population(DatasetAvailability::Absent, 0),
        "an undeclared definition family remains absent",
    );
    assert!(
        !known_population(DatasetAvailability::Absent, 1),
        "no authored row belongs to an absent family",
    );
}

#[test]
fn component_definition_parity_uses_each_exact_physical_header() -> TestResult {
    use serde_json::json;

    let flat = json!({"recordIndex":{"state":"present","value":7}});
    let nested = json!({"source":{"recordIndex":{"state":"present","value":11}}});
    for family in [Family::ConsumableTypes, Family::ConsumableSubtypes] {
        assert_eq!(
            family.owner().physical_index(&flat)?,
            7,
            "{}",
            family.path()
        );
        assert!(
            family.owner().physical_index(&nested).is_err(),
            "a nested header cannot replace the flat identity for {}",
            family.path(),
        );
    }
    for family in [
        Family::DamageResistanceMacros,
        Family::MiningLaserGlobals,
        Family::TurretHealthModifiers,
    ] {
        assert_eq!(
            family.owner().physical_index(&nested)?,
            11,
            "{}",
            family.path()
        );
        assert!(
            family.owner().physical_index(&flat).is_err(),
            "a flat header cannot replace the nested identity for {}",
            family.path(),
        );
    }
    Ok(())
}
