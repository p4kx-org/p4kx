// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::io::Read;

use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry, Entry, Guid, SourceEncryption};

use crate::starmap_xml::{Document, Node};

use super::TestResult;
use super::oracle::{AuthoredChild, reference_routes};

const SYSTEM_ROOT: &str = "data/objectcontainers/pu/system/";
const TILE_ROOT: &str = "data/objectcontainers/pu/starchitect/tile/";

pub(super) struct Witness<'archive> {
    pub(super) system: &'archive Entry,
    pub(super) body: &'archive Entry,
    pub(super) tile: &'archive Entry,
    pub(super) record: Guid,
    pub(super) body_route: Vec<AuthoredChild>,
    pub(super) tile_route: Vec<AuthoredChild>,
    pub(super) system_member: usize,
    pub(super) body_member: usize,
}

pub(super) fn find_witness<'archive>(
    archive: &'archive Archive,
    watch: &ArchiveWatch,
) -> TestResult<Option<Witness<'archive>>> {
    let entries = archive
        .entries()
        .iter()
        .filter_map(|entry| {
            let path = normalized(entry.path());
            path.ends_with(".socpak").then_some((path, entry))
        })
        .collect::<HashMap<_, _>>();
    let mut systems: Vec<_> = entries
        .iter()
        .filter(|(path, _)| has_system_root_name(path))
        .map(|(path, entry)| (path.as_str(), *entry))
        .collect();
    systems.sort_unstable_by_key(|&(path, _)| path);
    for (_, system) in systems {
        watch.poll()?;
        if let Some(found) = in_system(system, archive, &entries)? {
            verify_physical_record(archive, found.record)?;
            return Ok(Some(found));
        }
    }
    Ok(None)
}

fn verify_physical_record(archive: &Archive, guid: Guid) -> TestResult {
    let data = DataForgeEntry::read(archive)?;
    let forge = data.open()?;
    let record = forge
        .record_by_guid(guid)
        .ok_or("authored StarMapObject GUID")?;
    assert!(
        record.type_name()?.eq_ignore_ascii_case("StarMapObject")
            && record
                .record_path()?
                .replace('\\', "/")
                .to_ascii_lowercase()
                .starts_with("libs/foundry/records/starmap/pu/"),
        "the original physical Record is in the normal catalog selection"
    );
    Ok(())
}

fn in_system<'archive>(
    system: &'archive Entry,
    archive: &Archive,
    entries: &HashMap<String, &'archive Entry>,
) -> TestResult<Option<Witness<'archive>>> {
    let Some(system_source) = named_descriptor(archive, system)? else {
        return Ok(None);
    };
    let document = Document::parse(&system_source.bytes)?;
    for body_route in reference_routes(&document, system_source.encrypted)? {
        let Some(record) = body_route.record else {
            continue;
        };
        let Some((body_path, body)) = resolved_target(entries, body_route.target) else {
            continue;
        };
        if !body_path.starts_with(SYSTEM_ROOT) || has_system_root_name(body_path) {
            continue;
        }
        let Some(body_source) = named_descriptor(archive, body)? else {
            continue;
        };
        let body_document = Document::parse(&body_source.bytes)?;
        for tile_route in reference_routes(&body_document, body_source.encrypted)? {
            let Some((tile_path, tile)) = resolved_target(entries, tile_route.target) else {
                continue;
            };
            if !tile_path.starts_with(TILE_ROOT) {
                continue;
            }
            let Some(tile_source) = named_descriptor(archive, tile)? else {
                continue;
            };
            let tile_document = Document::parse(&tile_source.bytes)?;
            if !tile_document
                .root()
                .name()
                .eq_ignore_ascii_case("ObjectContainer")
            {
                continue;
            }
            if !body_route.has_pose() || !tile_route.has_pose() {
                continue;
            }
            return Ok(Some(Witness {
                system,
                body,
                tile,
                record,
                body_route: body_route.nodes,
                tile_route: tile_route.nodes,
                system_member: system_source.index,
                body_member: body_source.index,
            }));
        }
    }
    Ok(None)
}

fn resolved_target<'entries, 'archive>(
    entries: &'entries HashMap<String, &'archive Entry>,
    target: &str,
) -> Option<(&'entries str, &'archive Entry)> {
    let normalized = normalized(target);
    let extension = if normalized.ends_with(".socpak") {
        ""
    } else {
        ".socpak"
    };
    ["", "data/", "data/objectcontainers/"]
        .into_iter()
        .map(|prefix| format!("{prefix}{normalized}{extension}"))
        .find_map(|key| {
            entries
                .get_key_value(&key)
                .map(|(path, entry)| (path.as_str(), *entry))
        })
}

struct Descriptor {
    bytes: Vec<u8>,
    index: usize,
    encrypted: Option<bool>,
}

fn named_descriptor(archive: &Archive, entry: &Entry) -> TestResult<Option<Descriptor>> {
    let normalized = normalized(entry.path());
    let stem = normalized
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".socpak"))
        .ok_or("outer container filename")?;
    let expected = format!("{stem}.xml");
    let opened = archive.open_container(entry)?;
    let mut found = opened.entries().iter().filter(|member| {
        let path = self::normalized(member.path());
        path.rsplit('/').next() == Some(expected.as_str())
            && !path.split('/').any(|part| part == "hcd")
    });
    let Some(descriptor) = found.next() else {
        return Ok(None);
    };
    if found.next().is_some() {
        return Err("original descriptor is ambiguous".into());
    }
    let mut bytes = Vec::new();
    opened
        .open_entry_verified(descriptor)?
        .read_to_end(&mut bytes)?;
    let original = opened
        .entry_origin(descriptor)
        .ok_or("exact archive Entry origin")?;
    let encrypted = match original.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    Ok(Some(Descriptor {
        bytes,
        encrypted,
        index: descriptor.index(),
    }))
}

fn has_system_root_name(path: &str) -> bool {
    let Some((folder, file)) = path
        .strip_prefix(SYSTEM_ROOT)
        .and_then(|rest| rest.split_once('/'))
    else {
        return false;
    };
    file == format!("{folder}system.socpak")
}

fn normalized(path: &str) -> String {
    path.trim_start_matches(['/', '\\'])
        .replace('\\', "/")
        .to_ascii_lowercase()
}
