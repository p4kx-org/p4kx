// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded installed witness for the authored binary SOC scene output.

use std::io::Read;

use p4kx_assembly::SocContainerCapture;
use p4kx_blender::SocScene;
use p4kx_chunks::{ByteOrder, ChunkFile, ChunkType, Container};
use p4kx_extraction::{Archive, ArchiveWatch, CancelToken, Entry};
use p4kx_testkit as testkit;
use serde_json::Value;

#[path = "soc_included_installed/geometry.rs"]
mod geometry;
#[path = "soc_included_installed/original.rs"]
mod original;
#[path = "soc_included_installed/wire.rs"]
mod wire;

use self::original::Original;

type Check<T> = Result<T, Box<dyn std::error::Error>>;

const MAX_PARENT_BYTES: u64 = 512 * 1024;
const MAX_CANDIDATE_PARENTS: usize = 300;
const INCLUDED_OBJECTS: ChunkType = ChunkType::new(Container::CrCh, 0x0010);

fn projected_table<'a>(scene: &'a Value, stream: &Value, ordinal: usize) -> Check<&'a Value> {
    let tables = wire::array(scene, "includedObjects")?;
    wire::array(stream, "includedObjects")?
        .iter()
        .filter_map(|index| {
            usize::try_from(index.as_u64()?)
                .ok()
                .and_then(|index| tables.get(index))
        })
        .find(|table| table["ordinal"] == ordinal)
        .ok_or_else(|| "missing physical included-object table reference".into())
}

fn verify_selected(
    archive: &Archive,
    container: &Archive,
    parent: &Entry,
    entry: &Entry,
    original: &Original<'_>,
) -> Check<bool> {
    let cancel = CancelToken::new();
    let capture = SocContainerCapture::read(&[archive, container], &cancel)?;
    let occurrence = capture
        .occurrences()
        .first()
        .ok_or("captured container occurrence")?;
    let stream_index = occurrence.streams.iter().position(|stream| {
        stream
            .member
            .as_ref()
            .and_then(|selection| selection.selected())
            .is_some_and(|member| member.origin().entry().index() == entry.index())
    });
    let Some(stream_index) = stream_index else {
        return Ok(false);
    };
    let projection = SocScene::from_archive_capture(&capture, archive, &cancel)?;
    let value = serde_json::to_value(projection)?;
    let stream = value
        .pointer(&format!("/occurrences/0/streams/{stream_index}"))
        .ok_or("projected stream occurrence")?;
    let table = projected_table(&value, stream, original.ordinal)?;
    wire::compare(&value, table, container, parent, entry, original)?;
    geometry::compare(
        &value,
        stream,
        &table["source"],
        [container, archive],
        parent,
        original,
    )
}

fn verify_candidate(archive: &Archive, parent: &Entry) -> Check<bool> {
    let container = archive.open_container(parent)?;
    for entry in container
        .entries()
        .iter()
        .filter(|entry| entry.path().to_ascii_lowercase().ends_with(".soc"))
    {
        let mut bytes = Vec::new();
        container
            .open_entry_verified(entry)?
            .reject_overlong()
            .read_to_end(&mut bytes)?;
        let parsed = ChunkFile::parse(bytes)?;
        for (ordinal, header) in parsed.chunks_of(INCLUDED_OBJECTS) {
            if header.byte_order() != ByteOrder::Little {
                continue;
            }
            let original = Original::read(
                parsed.chunk_data(ordinal).ok_or("original member chunk")?,
                header,
                ordinal,
            )?;
            if original.name_ranges.iter().any(std::ops::Range::is_empty)
                || original.placement_rows()?.is_empty()
            {
                continue;
            }
            if verify_selected(archive, &container, parent, entry, &original)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn verify_archive(archive: &Archive) -> Check<()> {
    let watch = ArchiveWatch::new(archive);
    let mut candidates: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| {
            let path = entry.path().to_ascii_lowercase().replace('\\', "/");
            path.starts_with("data/objectcontainers/ships/")
                && path.ends_with(".socpak")
                && entry.uncompressed_size() <= MAX_PARENT_BYTES
        })
        .collect();
    candidates.sort_unstable_by_key(|entry| (entry.uncompressed_size(), entry.index()));
    for entry in candidates.into_iter().take(MAX_CANDIDATE_PARENTS) {
        watch.poll()?;
        if verify_candidate(archive, entry)? {
            watch.check()?;
            return Ok(());
        }
    }
    Err("selected source scope did not yield a table with all three path classes".into())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn both_installed_archives_project_original_included_object_bytes() -> Check<()> {
    let Some(pair) =
        testkit::real_archive_pair().map_err(|_| "configure the PTU and LIVE Archives")?
    else {
        return Err("configure the PTU and LIVE Archives".into());
    };
    let key = testkit::real_archive_key().map_err(|_| "read configured Archive key")?;
    for path in <[_; 2]>::from(pair) {
        let archive = Archive::open_with_key(path, Some(key.clone()))
            .map_err(|_| "open configured Archive")?;
        verify_archive(&archive).map_err(|_| "installed SOC wire reconciliation failed")?;
    }
    Ok(())
}
