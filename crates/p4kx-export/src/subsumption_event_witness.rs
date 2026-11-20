// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read as _;
use std::path::Path;
use std::time::UNIX_EPOCH;

use p4kx_extraction::cryxml::{self, Element};
use p4kx_extraction::{Archive, ArchiveWatch, Filter, Guid};
use p4kx_testkit::{real_archive_key, real_archive_pair};
use serde_json::Value;

use super::{TestResult, without_registry_ids};
use crate::CatalogOptions;
use crate::catalog_queries::subsumption_events::{
    SubsumptionEvent, SubsumptionEventDocument, SubsumptionEventLookup, SubsumptionEventSelection,
    SubsumptionEventSnapshot, read_subsumption_events,
};
use crate::catalog_read::original::with_entry_capture;
use crate::catalog_v3::subsumption_event;
use crate::export_ledger::ExportLedger;
use crate::starmap_index;

struct Observation {
    id: Guid,
    name: String,
    unchanged_attributes: Vec<(String, String)>,
    controls: Value,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_subsumption_event_rename_matches_source_and_staged_producer() -> TestResult {
    let Some((current, previous)) = real_archive_pair()? else {
        return Ok(());
    };
    let old_entry = selector("P4KX_SUBSUMPTION_OLD_ENTRY")?;
    let new_entry = selector("P4KX_SUBSUMPTION_NEW_ENTRY")?;
    if starmap_index::key(&old_entry) == starmap_index::key(&new_entry) {
        return Err("expected different exact physical source paths".into());
    }
    let before = observe(&previous, &old_entry)?;
    let after = observe(&current, &new_entry)?;
    if before.id != after.id || before.name == after.name {
        return Err("expected the same authored ID with different names".into());
    }
    if before.unchanged_attributes != after.unchanged_attributes
        || before.controls != after.controls
    {
        return Err("event declarations differ beyond their original symbol".into());
    }
    Ok(())
}

fn selector(variable: &str) -> TestResult<String> {
    let path = std::env::var(variable).map_err(|_| "set both exact event selectors")?;
    if path.is_empty() || path.contains(['*', '?']) {
        return Err("event selector must be a nonempty literal path".into());
    }
    Ok(path)
}

fn observe(path: &Path, requested: &str) -> TestResult<Observation> {
    let archive = Archive::open_with_key(
        path,
        Some(real_archive_key().map_err(|_| "archive key unavailable")?),
    )
    .map_err(|_| "selected archive could not open")?;
    let watch = ArchiveWatch::new(&archive);
    watch.check().map_err(|_| "selected archive changed")?;
    let snapshot = read_subsumption_events(
        &archive,
        SubsumptionEventSelection::ArchivePath(requested),
        None,
    )
    .map_err(|_| "selected event SDK query could not complete")?;
    let [document] = snapshot.rows.as_slice() else {
        return Err("exact path did not produce one physical document".into());
    };
    if snapshot.lookup != Some(SubsumptionEventLookup::Unique) || snapshot.entries.selected != 1 {
        return Err("exact path lookup did not establish a complete single entry".into());
    }
    let observation = read_original(&archive, requested, &snapshot, document)?;
    compare_staged(&archive, requested, document)?;
    watch.check().map_err(|_| "selected archive changed")?;
    Ok(observation)
}

fn read_original(
    archive: &Archive,
    requested: &str,
    snapshot: &SubsumptionEventSnapshot,
    document: &SubsumptionEventDocument,
) -> TestResult<Observation> {
    let key = starmap_index::key(requested);
    let mut candidates = archive
        .entries()
        .iter()
        .filter(|entry| starmap_index::key(entry.path()) == key);
    let entry = candidates
        .next()
        .ok_or("exact event source entry missing")?;
    if candidates.next().is_some()
        || u32::try_from(entry.index())? != document.identity().entry_index()
    {
        return Err("SDK physical entry disagrees with the exact archive selector".into());
    }
    let mut bytes = Vec::new();
    let count = archive
        .open_entry_verified(entry)
        .map_err(|_| "verified event stream could not open")?
        .reject_overlong()
        .read_to_end(&mut bytes)
        .map_err(|_| "event extent or checksum was not verified")?;
    if u64::try_from(count)? != entry.uncompressed_size() {
        return Err("event extent disagrees with the original directory".into());
    }
    let decoded = cryxml::parse(&bytes).map_err(|_| "original event CryXml could not decode")?;
    let root = decoded.root();
    let mut events = root.children().filter(|node| node.name() == "Event");
    let original = events.next().ok_or("direct original event missing")?;
    if root.name() != "Subsumption" || events.next().is_some() {
        return Err("expected one event in the selected original document".into());
    }
    compare_original(snapshot, document, original)
}

fn compare_original(
    snapshot: &SubsumptionEventSnapshot,
    document: &SubsumptionEventDocument,
    original: Element<'_>,
) -> TestResult<Observation> {
    let [event] = document.events() else {
        return Err("one typed event expected".into());
    };
    let index = usize::try_from(
        *event
            .element_index()
            .value()
            .ok_or("typed event position")?,
    )?;
    let element = document
        .elements()
        .get(index)
        .ok_or("original event element")?;
    let authored = element.declarations().iter().map(|cell| {
        (
            cell.name().value().map(String::as_str),
            cell.value().value().value().map(String::as_str),
        )
    });
    if !authored.eq(original
        .attributes()
        .map(|cell| (Some(cell.name()), Some(cell.value()))))
    {
        return Err("SDK does not preserve original declarations in their stored order".into());
    }
    let id = attribute(original, "ID")?
        .trim_matches(['{', '}'])
        .parse()?;
    let name = attribute(original, "Name")?.to_owned();
    if event.id().value().map(|value| value.get()) != Some(id)
        || event.name().value() != Some(&name)
    {
        return Err("typed identity does not match the exact original attributes".into());
    }
    if !identity_origin_closes(snapshot, document, event) {
        return Err(
            "typed identity references do not close within its exact physical source".into(),
        );
    }
    Ok(Observation {
        id,
        name,
        unchanged_attributes: original
            .attributes()
            .filter(|cell| cell.name() != "Name")
            .map(|cell| (cell.name().to_owned(), cell.value().to_owned()))
            .collect(),
        controls: control_values(event)?,
    })
}

fn identity_origin_closes(
    snapshot: &SubsumptionEventSnapshot,
    document: &SubsumptionEventDocument,
    event: &SubsumptionEvent,
) -> bool {
    let uses = event.id().origin().source_uses_refs();
    !uses.is_empty()
        && uses.iter().all(|reference| {
            snapshot.source_inventory.iter_uses().any(|source_use| {
                source_use.id() == reference
                    && source_use.source() == document.identity().input_ref()
            })
        })
}

fn attribute<'document>(element: Element<'document>, key: &str) -> TestResult<&'document str> {
    let original = element
        .attributes()
        .find(|cell| cell.name() == key)
        .ok_or("original event attribute missing")?;
    Ok(original.value())
}

fn compare_staged(
    archive: &Archive,
    path: &str,
    document: &SubsumptionEventDocument,
) -> TestResult {
    let filter = Filter::new(path);
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.filter = Some(&filter);
    let folder = tempfile::tempdir()?;
    let staged = with_entry_capture(archive, |capture| {
        subsumption_event::stage(
            archive,
            capture,
            &options,
            &mut ExportLedger::default(),
            folder.path(),
        )
    })
    .map_err(|_| "scoped Catalog event projection could not complete")?;
    let produced: Value = serde_json::from_slice(&serde_json::to_vec(&staged)?)?;
    let expected = serde_json::from_slice(&serde_json::to_vec(&[document])?)?;
    if produced.get("rows").cloned().map(without_registry_ids)
        != Some(without_registry_ids(expected))
    {
        return Err(
            "scoped Catalog values or immediate source states differ from the exact SDK row".into(),
        );
    }
    Ok(())
}

fn control_values(event: &SubsumptionEvent) -> TestResult<Value> {
    let fields: Vec<_> = event
        .controls()
        .iter()
        .map(|control| {
            (
                control.field(),
                control.value().state(),
                control.value().value(),
            )
        })
        .collect();
    Ok(serde_json::to_value(fields)?)
}
