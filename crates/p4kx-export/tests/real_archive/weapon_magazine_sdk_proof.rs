// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_export::catalog_queries::equipment::{WeaponMagazineOutcome, WeaponMagazineSnapshot};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession, ProvenanceMode};
use p4kx_extraction::{
    Archive, ArchiveByteRange, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, DataType,
    Filter, InputScope, PhysicalSourceKind, RecordTree, SourceEncryption, SourceInput, SourceUses,
    StructValue, TreeBuilder, Value as Raw,
};
use serde_json::Value;

use crate::real_archive_item_components::selected_item;

use super::original::{self, WEAPON};
use super::{TestResult, Trace, cell, proof, with_sdk_archive_samples};

#[path = "weapon_magazine_container_replay.rs"]
pub(crate) mod ammo_container_replay;

const ENTRY_TYPE: &str = "EntityClassDefinition";
const CONTAINER: &str = "SAmmoContainerComponentParams";
const PRIMARY: &str = "ammoContainerRecord";

struct Witness<'source> {
    owner: RecordTree<'source>,
    component: usize,
    magazine: Option<RecordTree<'source>>,
    slot: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn weapon_magazine_sdk_matches_original_primary_routes() -> TestResult {
    with_sdk_archive_samples(
        "primary magazine",
        "one own container and one linked physical magazine, both exact original slots and origins",
        verify,
    )
}

fn verify(archive: &Archive) -> TestResult {
    let watch = ArchiveWatch::new(archive);
    let entry = DataForgeEntry::read(archive).map_err(|_| "read actual DataForge")?;
    let forge = entry.open().map_err(|_| "open actual DataForge")?;
    let witnesses = find(&forge)?;
    watch
        .check()
        .map_err(|_| "Archive changed during primary route selection")?;
    let session = CatalogQuerySession::from_entry(
        archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "bind the decoded DataForge to the magazine SDK")?
    .with_provenance_mode(ProvenanceMode::Full);
    let parsed = session
        .parsed(None)
        .map_err(|_| "open parsed magazine SDK session")?;
    for witness in witnesses {
        parsed
            .with_weapon_magazine_resolutions(witness.owner.record_index(), None, |snapshot| {
                if snapshot.build() != archive.build() {
                    return Err("the magazine SDK must preserve the exact input Build".into());
                }
                inspect(&snapshot, &entry, &witness)
            })
            .map_err(|_| "finish Archive-bound magazine SDK read")??;
    }
    watch
        .check()
        .map_err(|_| "Archive changed during magazine verification".into())
}

fn find<'source>(forge: &'source DataForge<'source>) -> TestResult<[Witness<'source>; 2]> {
    let rule = Kind::Items.rule().ok_or("original Item admission rule")?;
    let paths = Filter::new("/weapons/");
    let probe = forge
        .pointer_probe("Components", WEAPON)
        .ok_or("original weapon component declaration")?;
    let mut trees = TreeBuilder::new();
    let mut own = None;
    let mut linked = None;
    for record in forge.records() {
        if !matches!(selected_item(&record, &rule, &paths), Ok(true))
            || !matches!(trees.holds(&probe, &record), Ok(true))
        {
            continue;
        }
        let Ok(owner) = trees.build(&record) else {
            continue;
        };
        let Some((component, weapon)) = original::unique(&owner, WEAPON) else {
            continue;
        };
        if let Some((slot, _)) = original::unique(&owner, CONTAINER) {
            if own.is_none() {
                own = Some(Witness {
                    owner,
                    component,
                    magazine: None,
                    slot,
                });
            }
        } else if linked.is_none()
            && let Some(weapon) = original::structure(weapon)
            && let Some((magazine, slot)) = linked_container(&owner, weapon, &mut trees)
        {
            linked = Some(Witness {
                owner,
                component,
                magazine: Some(magazine),
                slot,
            });
        }
        if own.is_some() && linked.is_some() {
            break;
        }
    }
    Ok([
        own.ok_or("one readable original weapon with its own primary container")?,
        linked.ok_or("one readable original weapon with a linked primary magazine")?,
    ])
}

fn linked_container<'source>(
    owner: &RecordTree<'source>,
    weapon: &StructValue<'source>,
    trees: &mut TreeBuilder<'source>,
) -> Option<(RecordTree<'source>, usize)> {
    let raw = original::array(owner.field("Components")?)?;
    if raw
        .iter()
        .any(|value| original::structure(value).is_some_and(|node| node.type_name() == CONTAINER))
    {
        return None;
    }
    let caller = weapon
        .fields()
        .iter()
        .find(|field| field.name() == PRIMARY)?;
    if caller.storage().data_type() != DataType::Reference || caller.storage().is_array() {
        return None;
    }
    let Raw::Reference(reference) = caller.value() else {
        return None;
    };
    let target = reference.original_target();
    if target.record_type() != Some(ENTRY_TYPE) || target.record_index() == owner.record_index() {
        return None;
    }
    let record = target.record().ok()?;
    if !target.shares_source_with(&record)
        || record.index() != target.record_index()
        || record.guid() != target.record_guid()
    {
        return None;
    }
    let magazine = trees.build(&record).ok()?;
    let (slot, _) = original::unique(&magazine, CONTAINER)?;
    (magazine.is_from(&record) && magazine.type_name() == ENTRY_TYPE).then_some((magazine, slot))
}

fn inspect(
    snapshot: &WeaponMagazineSnapshot<'_>,
    entry: &DataForgeEntry,
    witness: &Witness<'_>,
) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one canonical primary resolution for the original physical Item".into());
    };
    let coverage = snapshot.coverage();
    if row.owner().record_index().value() != Some(&witness.owner.record_index())
        || row.owner().component_ordinal().value().copied()
            != Some(u32::try_from(witness.component)?)
        || !row.outcome().origin().source_set_complete()
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
        || !snapshot.requested_languages().is_empty()
        || !snapshot.available_languages().is_empty()
    {
        return Err(
            "the magazine SDK result must describe the exact readable route without locale inputs"
                .into(),
        );
    }
    check_outcome(row.outcome().value(), witness)?;
    check_provenance(snapshot, entry, witness, &serde_json::to_value(row)?)
}

fn check_outcome(actual: Option<&WeaponMagazineOutcome>, witness: &Witness<'_>) -> TestResult {
    let expected_slot = u32::try_from(witness.slot)?;
    let matches = match (actual, &witness.magazine) {
        (Some(WeaponMagazineOutcome::OwnContainer { container_ordinal }), None) => {
            container_ordinal.value() == Some(&expected_slot)
        }
        (
            Some(WeaponMagazineOutcome::LinkedContainer {
                magazine_record_index,
                container_ordinal,
            }),
            Some(target),
        ) => {
            magazine_record_index.value() == Some(&target.record_index())
                && container_ordinal.value() == Some(&expected_slot)
        }
        _ => false,
    };
    if !matches {
        return Err("the observed route must retain the original container's exact physical owner and position".into());
    }
    Ok(())
}

fn check_provenance(
    snapshot: &WeaponMagazineSnapshot<'_>,
    entry: &DataForgeEntry,
    witness: &Witness<'_>,
    row: &Value,
) -> TestResult {
    let DataForgeOrigin::Archive(origin) = entry.origin() else {
        return Err("the verified DataForge must have an Archive origin".into());
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(origin), InputScope::Payload)?;
    check_directory_entry(snapshot, scope.source_ref(), origin)?;
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let inventory = serde_json::to_value(snapshot.source_inventory())?;
    let uses = proof::indexed(&inventory, "/uses", "id")?;
    let owner = Trace::of(&witness.owner, &uses, &scope, encrypted);
    let (component, weapon) =
        original::unique(&witness.owner, WEAPON).ok_or("the original weapon component")?;
    if component != witness.component
        || proof::text(row, "/ownerGuid/value")? != witness.owner.guid().to_string()
    {
        return Err("the owner row must retain the original weapon component and its GUID".into());
    }
    owner.header(cell(row, "ownerRecordIndex")?, "recordIndex", "selector")?;
    owner.header(cell(row, "ownerGuid")?, "guid", "payload")?;
    owner.check(cell(row, "componentOrdinal")?, weapon, "conversion")?;
    let outcome = cell(row, "outcome")?;
    let output = cell(outcome, "value")?;
    let container = witness.magazine.as_ref().unwrap_or(&witness.owner);
    let (slot, original) =
        original::unique(container, CONTAINER).ok_or("the original container component")?;
    if slot != witness.slot {
        return Err("the original primary container position must remain stable".into());
    }
    let target = Trace::of(container, &uses, &scope, encrypted);
    target.check(cell(output, "containerOrdinal")?, original, "conversion")?;
    target.part(outcome, original, "$type", "selector")?;
    if witness.magazine.is_some() {
        let caller = original::structure(weapon)
            .and_then(|component| component.field(PRIMARY))
            .ok_or("the original linked magazine Reference")?;
        let index = cell(output, "magazineRecordIndex")?;
        owner.check(index, caller, "selector")?;
        target.header(index, "recordIndex", "selector")?;
        owner.check(outcome, caller, "selector")?;
        target.header(outcome, "sourceType", "selector")?;
    }
    Ok(())
}

fn check_directory_entry(
    snapshot: &WeaponMagazineSnapshot<'_>,
    source_id: &p4kx_extraction::PhysicalSourceRef,
    origin: &p4kx_extraction::EntryOrigin,
) -> TestResult {
    let physical = snapshot
        .source_inventory()
        .iter_sources()
        .find(|source| source.id() == source_id)
        .ok_or("the original DataForge Entry is present in the scoped source inventory")?;
    let original = origin.entry();
    let exact = match physical.kind() {
        PhysicalSourceKind::ArchiveEntry(entry) => {
            entry.entry_index() == original.index()
                && entry
                    .path()
                    .value()
                    .is_some_and(|path| path == original.path())
                && entry.record_range() == ArchiveByteRange::of(original.metadata().record_range())
        }
        PhysicalSourceKind::ArchiveEntryProof(entry) => {
            entry.entry_index().get() == u64::from(original.ordinal())
        }
        _ => false,
    };
    if !exact {
        return Err("the route's physical DataForge Entry identity must match the original directory evidence".into());
    }
    Ok(())
}
