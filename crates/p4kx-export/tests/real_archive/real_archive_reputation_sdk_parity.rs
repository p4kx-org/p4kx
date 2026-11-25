// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::{
    CatalogGuid, CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
    ParsedCatalogQueries, ReputationSelection, ReputationSnapshot,
};
use p4kx_extraction::{SourceInventory, SourceLocation};

use crate::real_archive_catalog_table_parity::{TestResult, with_bound_measurement_from_producer};
use crate::real_archive_scoped_equipment_sdk::equipment_input;
use crate::real_archive_verification::print_line;

#[path = "real_archive_reputation_sdk_evidence.rs"]
mod evidence;
use evidence::{Localized, Position, Proof, Snapshot};

const COUNT: usize = 10;
const PRODUCER: &str = "P4KX_REPUTATION_CATALOG_PRODUCER_REVISION";
const COLD_DATASET: &str = "P4KX_REPUTATION_SDK_COLD_DATASET";
const EXPECTED_ROWS: &str = "P4KX_REPUTATION_SDK_EXPECTED_ROWS";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Family {
    FactionReputations,
    Scopes,
    Standings,
    Contexts,
    Perks,
    TriggerRecords,
    Triggers,
    Rewards,
    Factions,
    Identities,
}

const FAMILIES: [Family; COUNT] = [
    Family::FactionReputations,
    Family::Scopes,
    Family::Standings,
    Family::Contexts,
    Family::Perks,
    Family::TriggerRecords,
    Family::Triggers,
    Family::Rewards,
    Family::Factions,
    Family::Identities,
];

const SUPPLEMENTARY: [Family; 4] = [
    Family::Triggers,
    Family::Rewards,
    Family::Factions,
    Family::Identities,
];

impl Family {
    const fn path(self) -> &'static str {
        match self {
            Self::FactionReputations => "/definitions/factionReputations",
            Self::Scopes => "/definitions/reputationScopes",
            Self::Standings => "/definitions/reputationStandings",
            Self::Contexts => "/definitions/reputationContexts",
            Self::Perks => "/definitions/reputationPerks",
            Self::TriggerRecords => "/definitions/reputationTriggerRecords",
            Self::Triggers => "/configurations/reputationTriggers",
            Self::Rewards => "/definitions/reputationRewardAmounts",
            Self::Factions => "/definitions/factions",
            Self::Identities => "/definitions/factionIdentities",
        }
    }

    const fn languages(self) -> CatalogLanguageSelection<'static> {
        match self {
            Self::TriggerRecords | Self::Triggers | Self::Rewards => {
                CatalogLanguageSelection::Selected(&[])
            }
            _ => CatalogLanguageSelection::AllAvailable,
        }
    }

    fn reputation_selection(self) -> Option<ReputationSelection> {
        let mut selection = ReputationSelection::default();
        let request = match self {
            Self::FactionReputations => &mut selection.faction_reputations,
            Self::Scopes => &mut selection.scopes,
            Self::Standings => &mut selection.standings,
            Self::Contexts => &mut selection.contexts,
            Self::Perks => &mut selection.perks,
            Self::TriggerRecords => &mut selection.trigger_records,
            _ => return None,
        };
        *request = Some(CatalogRecordSelection::All);
        Some(selection)
    }
}

#[test]
#[ignore = "standalone cold query: configure the installed Build and one Reputation or Faction route"]
fn installed_reputation_and_factions_cold_query() -> TestResult {
    let path = env::var(COLD_DATASET)?;
    let family = FAMILIES
        .into_iter()
        .find(|family| family.path() == path)
        .ok_or("configure one exact Reputation or Faction dataset path")?;
    let expected = parse_counts(&env::var(EXPECTED_ROWS)?)?;
    equipment_input::with_archive(|archive| {
        let session = CatalogQuerySession::open(archive, family.languages(), None)?;
        let parsed = session.parsed(None)?;
        let mut proof = Proof::cold(archive, expected);
        if let Some(selection) = family.reputation_selection() {
            reputation(&parsed, selection, &mut proof)?;
        } else {
            supplementary(&parsed, family, &mut proof)?;
        }
        proof.finish_cold(family)
    })
}

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_reputation_and_factions_match_one_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    let expected = parse_counts(&env::var(EXPECTED_ROWS)?)?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let mut proof = Proof::retained(archive, expected);
        {
            let session = equipment_input::full_source_session(
                archive,
                CatalogLanguageSelection::AllAvailable,
            )?;
            let parsed = session.parsed(None)?;
            reputation(&parsed, ReputationSelection::all(), &mut proof)?;
            for family in SUPPLEMENTARY {
                supplementary(&parsed, family, &mut proof)?;
            }
        }
        let counts = proof.compare(measurement)?;
        print_line(&format!(
            "complete Reputation SDK parity: rows={:?} sourceUses={} physicalSources={}",
            counts.rows, counts.source_uses, counts.physical_sources,
        ));
        Ok(())
    })
}

fn reputation(
    parsed: &ParsedCatalogQueries<'_>,
    selection: ReputationSelection,
    proof: &mut Proof<'_>,
) -> TestResult {
    parsed.with_reputation(selection, None, |snapshot| {
        observe_reputation(&snapshot, proof)
    })?
}

fn observe_reputation(snapshot: &ReputationSnapshot<'_>, proof: &mut Proof<'_>) -> TestResult {
    proof.require_build(snapshot.build.as_ref())?;
    let inventory = &snapshot.source_inventory;
    macro_rules! family {
        ($name:ident, $rows:ident) => {
            if let Some(rows) = &snapshot.$rows {
                proof.observe(
                    Family::$name,
                    &Snapshot::reputation(rows, inventory),
                    |row| Position::record(row.source()),
                )?;
            }
        };
    }
    family!(FactionReputations, faction_reputations);
    family!(Scopes, scopes);
    family!(Standings, standings);
    family!(Contexts, contexts);
    family!(Perks, perks);
    family!(TriggerRecords, trigger_records);
    proof.observe_texts(
        &Localized {
            requested: &snapshot.requested_languages,
            available: &snapshot.available_languages,
            public: &snapshot.public_names,
            protected: &snapshot.game_texts,
        },
        inventory,
    )
}

fn supplementary(
    parsed: &ParsedCatalogQueries<'_>,
    family: Family,
    proof: &mut Proof<'_>,
) -> TestResult {
    let selection = CatalogRecordSelection::All;
    match family {
        Family::Triggers => parsed.with_reputation_triggers(selection, None, |sdk| {
            proof.require_build(sdk.build.as_ref())?;
            require_no_locale(&sdk.source_inventory)?;
            proof.observe(
                family,
                &Snapshot {
                    rows: &sdk.rows,
                    availability: sdk.availability,
                    coverage: &sdk.coverage,
                    inventory: &sdk.source_inventory,
                },
                |row| {
                    Position::new(
                        row.owner_guid()
                            .value()
                            .copied()
                            .map(CatalogGuid::get)
                            .as_ref(),
                        row.owner_record_index().value(),
                    )
                },
            )
        })?,
        Family::Rewards => parsed.with_reputation_reward_amounts(selection, None, |sdk| {
            proof.require_build(sdk.build.as_ref())?;
            require_no_locale(&sdk.source_inventory)?;
            proof.observe(
                family,
                &Snapshot {
                    rows: &sdk.rows,
                    availability: sdk.availability,
                    coverage: &sdk.coverage,
                    inventory: &sdk.source_inventory,
                },
                |row| {
                    row.reputation_amount()
                        .value()
                        .ok_or("missing original signed reward amount")?;
                    Position::record(row.source())
                },
            )
        })?,
        Family::Factions => observe_factions(parsed, proof),
        Family::Identities => observe_identities(parsed, proof),
        _ => Err("the route is supplied by the Reputation family query".into()),
    }
}

fn observe_factions(parsed: &ParsedCatalogQueries<'_>, proof: &mut Proof<'_>) -> TestResult {
    parsed.with_factions(CatalogRecordSelection::All, None, |sdk| {
        proof.require_build(sdk.build())?;
        proof.observe(
            Family::Factions,
            &Snapshot {
                rows: sdk.rows(),
                availability: sdk.availability(),
                coverage: sdk.coverage(),
                inventory: sdk.source_inventory(),
            },
            |row| Position::record(row.source()),
        )?;
        proof.observe_texts(
            &Localized {
                requested: sdk.requested_languages(),
                available: sdk.available_languages(),
                public: sdk.public_names(),
                protected: sdk.game_texts(),
            },
            sdk.source_inventory(),
        )
    })?
}

fn observe_identities(parsed: &ParsedCatalogQueries<'_>, proof: &mut Proof<'_>) -> TestResult {
    parsed.with_faction_identities(CatalogRecordSelection::All, None, |sdk| {
        proof.require_build(sdk.build.as_ref())?;
        proof.observe(
            Family::Identities,
            &Snapshot {
                rows: &sdk.rows,
                availability: sdk.availability,
                coverage: &sdk.coverage,
                inventory: &sdk.source_inventory,
            },
            |row| Position::record(row.source()),
        )?;
        proof.observe_texts(
            &Localized {
                requested: &sdk.requested_languages,
                available: &sdk.available_languages,
                public: &[],
                protected: &sdk.game_texts,
            },
            &sdk.source_inventory,
        )
    })?
}

fn require_no_locale(inventory: &SourceInventory) -> TestResult {
    if inventory
        .iter_uses()
        .any(|used| matches!(used.location(), SourceLocation::LocalizationLookup { .. }))
    {
        return Err("an authored reward or trigger query unexpectedly used localization".into());
    }
    Ok(())
}

fn parse_counts(value: &str) -> TestResult<[usize; COUNT]> {
    value
        .split(',')
        .map(|value| {
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err("invalid expected Reputation route row count".into());
            }
            Ok(value.parse()?)
        })
        .collect::<TestResult<Vec<_>>>()?
        .try_into()
        .map_err(|_| "configure ten ordered Reputation route row counts".into())
}

#[test]
fn reputation_route_scope_keeps_declared_empty_and_nonlocalized_routes_distinct() -> TestResult {
    let counts = parse_counts("39,46,380,27,2,3,39,58,60,0")?;
    assert_eq!(
        counts.last(),
        Some(&0),
        "zero identities is a valid expected count"
    );
    assert!(
        parse_counts("39,46,380").is_err(),
        "a partial route census cannot authorize the proof"
    );
    assert!(
        parse_counts("39,46,380,27,2,3,39,58,60,-1").is_err(),
        "counts cannot express field unavailability"
    );
    assert!(
        matches!(
            Family::Triggers.languages(),
            CatalogLanguageSelection::Selected(&[])
        ),
        "trigger configurations use no locale payloads"
    );
    assert!(
        matches!(
            Family::Identities.languages(),
            CatalogLanguageSelection::AllAvailable
        ),
        "the identity route uses the Normal locale policy even when empty"
    );
    Ok(())
}
