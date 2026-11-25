// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ContractKey,
    GameTextRef, ParsedCatalogQueries, ProvenanceMode, ReputationSelection, ReputationSnapshot,
    ResourceNumber,
};
use p4kx_extraction::{Archive, Guid, Language, SourceInventory, SourcedValue};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, test_key};

use super::{
    CONTRACT_A, FACTION, FACTION_SCOPE, database_with_scope, english_entry, english_strings,
};
use crate::commodity_v3_fixture::{TestResult, guid};
use crate::reputation_fixture::{self, STANDING_HIGH, STANDING_LOW};
use crate::sdk_full_proof as full_proof;

const LOCALIZED_RANK: &str = "Senior Delivery Specialist";
const RANK_KEY: &str = "fixture_delivery_rank";
const RANK_THRESHOLD: i64 = 4_500;

#[test]
fn default_marker_consumer_resolves_contract_reward_and_localized_rank() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .entry(english_entry(
            &english_strings().text(RANK_KEY, LOCALIZED_RANK),
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let languages = CatalogLanguageSelection::Selected(&[Language::English]);
    let marker = CatalogQuerySession::open(&archive, languages, None)?;
    let full = full_proof::open(&archive, languages, None)?;
    let expected = ConsumerView {
        faction: guid(FACTION).parse()?,
        scope: guid(FACTION_SCOPE).parse()?,
        reward: 200,
        threshold: RANK_THRESHOLD,
        faction_name: "Acme Emporium".to_owned(),
        rank_name: LOCALIZED_RANK.to_owned(),
    };

    assert_eq!(
        collect(&marker.parsed(None)?, ProvenanceMode::MarkersOnly)?,
        expected,
        "the default profile resolves the original contract and localized reputation values"
    );
    assert_eq!(
        collect(&full.parsed(None)?, ProvenanceMode::Full)?,
        expected,
        "the Full profile resolves the same original contract and localized reputation values"
    );
    Ok(())
}

fn database() -> DataForgeBuilder {
    let builder = reputation_fixture::structures::rank_structs()
        .into_iter()
        .fold(DataForgeBuilder::new(), DataForgeBuilder::structure);
    database_with_scope(builder, reputation_fixture::scope(FACTION_SCOPE))
        .record(reputation_fixture::standing(
            STANDING_LOW,
            "ReputationStanding_Faction_B0_00",
            0,
            false,
            -0.0,
            "@LOC_PLACEHOLDER",
        ))
        .record(reputation_fixture::standing(
            STANDING_HIGH,
            "ReputationStanding_Faction_B0_01",
            RANK_THRESHOLD,
            true,
            24.5,
            "@fixture_delivery_rank",
        ))
}

#[derive(Debug, PartialEq, Eq)]
struct ConsumerView {
    faction: Guid,
    scope: Guid,
    reward: i64,
    threshold: i64,
    faction_name: String,
    rank_name: String,
}

fn collect(parsed: &ParsedCatalogQueries<'_>, mode: ProvenanceMode) -> TestResult<ConsumerView> {
    let key = ContractKey::new(guid(CONTRACT_A).parse()?, None);
    let (faction, scope, reward) =
        parsed.with_contract(key, None, |snapshot| -> TestResult<_> {
            check_inventory(&snapshot.source_inventory, mode);
            let [contract] = snapshot.rows.as_slice() else {
                return Err("one exact parent contract".into());
            };
            assert_eq!(
                field(&contract.base.guid, mode)?.get(),
                key.contract_guid(),
                "the returned row has the requested contract identity"
            );
            assert!(
                contract.base.sub_contract_guid.value().is_none(),
                "the parent key selects the parent occurrence"
            );
            let wanted_scope = guid(FACTION_SCOPE).parse()?;
            let gain = field(&contract.base.reputation_gains, mode)?
                .iter()
                .find(|gain| {
                    gain.scope_guid
                        .value()
                        .is_some_and(|guid| guid.get() == wanted_scope)
                })
                .ok_or("the authored faction scope reward")?;
            Ok((
                field(&gain.faction_reputation_guid, mode)?.get(),
                field(&gain.scope_guid, mode)?.get(),
                field(&gain.amount, mode)?.get(),
            ))
        })??;
    let mut selection = ReputationSelection::ranks();
    selection.faction_reputations = Some(CatalogRecordSelection::Guid(faction));
    selection.scopes = Some(CatalogRecordSelection::Guid(scope));
    let (faction_name, rank_name, threshold) =
        parsed.with_reputation(selection, None, |snapshot| reached_rank(&snapshot, mode))??;
    Ok(ConsumerView {
        faction,
        scope,
        reward,
        threshold,
        faction_name,
        rank_name,
    })
}

fn reached_rank(
    snapshot: &ReputationSnapshot<'_>,
    mode: ProvenanceMode,
) -> TestResult<(String, String, i64)> {
    check_inventory(&snapshot.source_inventory, mode);
    let factions = &snapshot
        .faction_reputations
        .as_ref()
        .ok_or("faction family")?
        .rows;
    let [faction] = factions.as_slice() else {
        return Err("one reached faction".into());
    };
    let scopes = &snapshot.scopes.as_ref().ok_or("scope family")?.rows;
    let [scope] = scopes.as_slice() else {
        return Err("one reached scope".into());
    };
    let links = field(field(scope.standing_map(), mode)?.standings(), mode)?;
    let [_, second] = links.as_slice() else {
        return Err("both original rank slots".into());
    };
    let link = field(second, mode)?;
    let standing = snapshot.standing(link).ok_or("the linked rank")?;
    let expected_standing: Guid = guid(STANDING_HIGH).parse()?;
    assert_eq!(
        field(&standing.source().guid(), mode)?,
        &expected_standing,
        "the higher standing comes from the Record linked by the original scope"
    );

    let ResourceNumber::Int64 { value } = field(standing.progression().min_reputation(), mode)?
    else {
        return Err("the authored Int64 rank threshold".into());
    };
    Ok((
        localized(snapshot, faction.display_name(), mode)?,
        localized(snapshot, standing.presentation().display_name(), mode)?,
        value.get(),
    ))
}

fn localized(
    snapshot: &ReputationSnapshot<'_>,
    reference: &SourcedValue<GameTextRef>,
    mode: ProvenanceMode,
) -> TestResult<String> {
    let text = snapshot
        .game_text(field(reference, mode)?)
        .ok_or("reached protected text")?;
    let english = field(text.translations(), mode)?
        .get("en")
        .ok_or("English translation")?;
    Ok(field(&english.display_text, mode)?.to_string())
}

#[track_caller]
fn field<T>(value: &SourcedValue<T>, mode: ProvenanceMode) -> TestResult<&T> {
    let evidence = value.origin();
    assert_eq!(
        evidence.contains_encrypted_source(),
        Some(true),
        "the field retains its encrypted source marker in {mode:?}"
    );
    assert_eq!(
        evidence.source_uses_refs().is_empty(),
        mode == ProvenanceMode::MarkersOnly,
        "the field carries source references only with Full provenance"
    );
    value
        .value()
        .ok_or_else(|| "the original field value".into())
}

fn check_inventory(inventory: &SourceInventory, mode: ProvenanceMode) {
    assert_eq!(
        inventory.provenance_mode(),
        mode,
        "the result inventory uses the session's profile"
    );
    assert_eq!(
        inventory.source_count() == 0,
        mode == ProvenanceMode::MarkersOnly,
        "the result inventory carries physical sources only with Full provenance"
    );
    assert_eq!(
        inventory.use_count() == 0,
        mode == ProvenanceMode::MarkersOnly,
        "the result inventory carries physical source uses only with Full provenance"
    );
}
