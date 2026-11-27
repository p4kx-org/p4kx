// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::{BufReader, Seek};
use std::time::UNIX_EPOCH;

use p4kx_assembly::Entities;
use p4kx_export::{CatalogOptions, CatalogPublicationMode, Compression};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, Record, RecordTree, StructValue,
    Value as Original,
};
use serde::Deserialize;
use serde_json::Value;

use crate::real_archive_item_durability::{number, rows, text};
use crate::real_archive_verification;
use crate::real_archive_weapon_actions;

#[path = "weapon_reload/candidates.rs"]
mod candidates;
#[path = "weapon_reload/options.rs"]
mod options;
#[path = "weapon_reload/oracle.rs"]
mod oracle;
#[path = "weapon_reload/proof.rs"]
mod proof;
#[path = "weapon_reload/scopes.rs"]
mod scopes;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const OWNERS: &str = "P4KX_RELOAD_WITNESS_RECORD_INDICES";
const ANIMATED: &str = "SAnimatedCharacterComponentParams";
const ATTACHABLE: &str = "SAttachableComponentParams";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn player_mannequin_reload_options_match_their_physical_source() -> TestResult {
    let path = p4kx_testkit::real_archive_path().map_err(|_| "configured Archive")?;
    let key = p4kx_testkit::real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let dcb = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = dcb.open().map_err(|_| "open original DataForge")?;
    let entities = Entities::new(&forge);
    let [player] = entities.named_class_candidates("Player") else {
        return Err("the selected Archive has exactly one original Player class".into());
    };
    let player = player.tree().map_err(|_| "read original Player Record")?;
    let motion = motion(&player)?;
    let build = archive.build().ok_or("physical Archive Build identity")?;
    real_archive_verification::print_line(&format!(
        "reload oracle Build: {} {} {} {}",
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ));
    for (ordinal, record) in owners(&forge)?.into_iter().enumerate() {
        watch
            .poll()
            .map_err(|_| "Archive changed during comparison")?;
        let weapon = record.tree().map_err(|_| "read original weapon Record")?;
        compare(&archive, &dcb, &weapon, &player, &motion, ordinal)?;
    }
    watch
        .check()
        .map_err(|_| "Archive changed before final verification")?;
    Ok(())
}

fn owners<'forge>(forge: &'forge DataForge<'forge>) -> TestResult<[Record<'forge>; 2]> {
    if let Ok(indices) = std::env::var(OWNERS) {
        let indices = indices
            .split(',')
            .map(str::parse::<u32>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "expected two comma-separated physical weapon indices")?;
        let [first, second] = indices.as_slice() else {
            return Err("expected exactly two physical weapon indices".into());
        };
        if first == second {
            return Err("the two physical weapon indices must differ".into());
        }
        return Ok([
            forge.record(*first).ok_or("first physical weapon Record")?,
            forge
                .record(*second)
                .ok_or("second physical weapon Record")?,
        ]);
    }
    real_archive_weapon_actions::personal_witnesses(forge)
}

fn compare(
    archive: &Archive,
    dcb: &DataForgeEntry,
    weapon: &RecordTree<'_>,
    player: &RecordTree<'_>,
    motion: &Motion<'_>,
    witness: usize,
) -> TestResult {
    let filter = Filter::new(weapon.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let (summary, mut file) = p4kx_export::write_catalog_from(
        archive,
        dcb,
        options.with_full_provenance(),
        Compression::None,
        tempfile::tempfile,
    )
    .map_err(|_| "write the exact scoped weapon Catalog")?;
    real_archive_verification::print_line(&format!(
        "reload witness {}: Catalog written, {} JSON bytes",
        witness + 1,
        summary.size(),
    ));
    drop(summary);
    file.rewind()?;
    let catalog: CatalogSample =
        serde_json::from_reader(BufReader::new(file)).map_err(|_| "read Catalog JSON")?;
    real_archive_verification::print_line(&format!(
        "reload witness {}: Catalog parsed",
        witness + 1,
    ));
    let inventory = proof::Inventory::read(&catalog.provenance, archive)?;
    let attachment = weapon_attachment(weapon)?;
    let raw = &catalog.definitions.weapon_animation_sources;
    let weapon_uses = check_weapon(
        &catalog.definitions.items,
        &inventory,
        dcb,
        weapon,
        &attachment,
    )?;
    check_player(raw, &inventory, dcb, player, motion)?;
    let counts = oracle::compare(
        archive,
        raw,
        &catalog.projections.weapon_player_animation_candidates,
        &inventory,
        &attachment,
        motion,
        weapon_uses,
    )?;
    real_archive_verification::print_line(&format!("reload witness {}: {counts:?}", witness + 1));
    Ok(())
}

#[derive(Deserialize)]
struct CatalogSample {
    definitions: DefinitionsSample,
    projections: ProjectionsSample,
    provenance: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DefinitionsSample {
    items: Value,
    weapon_animation_sources: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectionsSample {
    weapon_player_animation_candidates: Value,
}

struct Attachment<'source> {
    index: u32,
    ordinal: usize,
    tags: &'source str,
}

struct Motion<'source> {
    index: u32,
    ordinal: usize,
    database: &'source str,
    controller: &'source str,
}

fn weapon_attachment<'source>(tree: &RecordTree<'source>) -> TestResult<Attachment<'source>> {
    let attachments = components(tree)?
        .filter(|(_, node)| node.type_name() == ATTACHABLE)
        .filter_map(|(ordinal, node)| match node.field("AttachDef") {
            Some(Original::Struct(definition))
                if label(definition, "Type") == Some("WeaponPersonal") =>
            {
                Some((ordinal, definition))
            }
            _ => None,
        });
    let (ordinal, definition) = unique(attachments, "unique original personal weapon attachment")?;
    Ok(Attachment {
        index: tree.record_index(),
        ordinal,
        tags: label(definition, "Tags").ok_or("original weapon tags")?,
    })
}

fn motion<'source>(tree: &RecordTree<'source>) -> TestResult<Motion<'source>> {
    let (ordinal, node) = unique(
        components(tree)?.filter(|(_, node)| node.type_name() == ANIMATED),
        "unique original Player animation component",
    )?;
    Ok(Motion {
        index: tree.record_index(),
        ordinal,
        database: label(node, "animDatabase3P").ok_or("original Player AnimDB selector")?,
        controller: label(node, "actionController").ok_or("original Player controller selector")?,
    })
}

fn components<'node, 'source>(
    tree: &'node RecordTree<'source>,
) -> TestResult<impl Iterator<Item = (usize, &'node StructValue<'source>)>> {
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original ordered Components array".into());
    };
    Ok(components
        .iter()
        .enumerate()
        .filter_map(|(ordinal, component)| {
            let Original::Struct(node) = component else {
                return None;
            };
            Some((ordinal, node))
        }))
}

fn label<'source>(node: &StructValue<'source>, field: &str) -> Option<&'source str> {
    match node.field(field) {
        Some(Original::String(value) | Original::Enum(value)) => Some(value),
        _ => None,
    }
}

fn unique<T>(mut rows: impl Iterator<Item = T>, message: &'static str) -> TestResult<T> {
    let first = rows.next().ok_or(message)?;
    if rows.next().is_some() {
        return Err(message.into());
    }
    Ok(first)
}

fn check_weapon<'catalog>(
    items: &'catalog Value,
    inventory: &proof::Inventory<'_>,
    dcb: &DataForgeEntry,
    tree: &RecordTree<'_>,
    attachment: &Attachment<'_>,
) -> TestResult<&'catalog [Value]> {
    let owner = unique(
        rows(items, "/rows")?.iter().filter(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64)
                == Some(u64::from(attachment.index))
        }),
        "one Catalog item definition for the physical weapon",
    )?;
    let component = unique(
        rows(owner, "/components/value")?.iter().filter(|row| {
            number(row, "/componentOrdinal/value").ok() == u64::try_from(attachment.ordinal).ok()
        }),
        "weapon attachment physical component",
    )?;
    let motion = component
        .pointer("/value/value/weaponMotion")
        .ok_or("weapon motion fields")?;
    let (_, actual) = components(tree)?
        .find(|(ordinal, _)| *ordinal == attachment.ordinal)
        .ok_or("original weapon attachment coordinate")?;
    let Some(Original::Struct(definition)) = actual.field("AttachDef") else {
        return Err("original AttachDef body".into());
    };
    assert!(
        text(motion, "/tags/value")? == attachment.tags,
        "authored attachment tags"
    );
    assert!(
        text(motion, "/itemType/value")? == "WeaponPersonal",
        "authored personal type"
    );
    inventory.dcb_fields(
        dcb,
        tree,
        definition,
        motion,
        &[("Tags", "tags"), ("Type", "itemType")],
    )?;
    rows(&motion["tags"], "/sourceUsesRefs")
}

fn check_player(
    raw: &Value,
    inventory: &proof::Inventory<'_>,
    dcb: &DataForgeEntry,
    tree: &RecordTree<'_>,
    motion: &Motion<'_>,
) -> TestResult {
    let player = unique(
        rows(raw, "/player/value")?.iter(),
        "one original Catalog Player",
    )?;
    assert!(
        number(player, "/recordIndex/value")? == u64::from(motion.index),
        "Player uses the original physical Record index",
    );
    let component = rows(player, "/components/value")?
        .get(motion.ordinal)
        .and_then(|slot| slot.get("value"))
        .ok_or("original Player component position")?;
    assert!(
        number(component, "/ordinal/value")? == u64::try_from(motion.ordinal)?,
        "Player component preserves its ordinal",
    );
    assert!(
        text(component, "/actionController/value")? == motion.controller
            && text(component, "/animDatabase3P/value")? == motion.database,
        "both original Player locators survive",
    );
    let (_, actual) = components(tree)?
        .find(|(ordinal, _)| *ordinal == motion.ordinal)
        .ok_or("original Player Animation component")?;
    inventory.dcb_fields(
        dcb,
        tree,
        actual,
        component,
        &[
            ("actionController", "actionController"),
            ("animDatabase3P", "animDatabase3P"),
        ],
    )
}
