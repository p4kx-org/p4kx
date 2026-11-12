// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_channels::PublicPatches;
use p4kx_extraction::{Archive, ArchiveLayoutInfo, Filter};

use crate::container::Place;
use crate::entry_search_origins;
use crate::error::ToolError;
use crate::folder_listing_origins::{FolderListingOrigins, FolderLookup};
use crate::page::Page;
use crate::shapes::{
    ArchiveInfo, BuildShape, EntryShape, FolderItem, FolderResult, IssueShape, SearchResult,
};

const MAX_SHOWN_ISSUES: usize = 50;

/// Returns the summary of `archive`, its Build with the public patch `patches` knows.
pub(crate) fn info(archive: &Archive, patches: &PublicPatches) -> ArchiveInfo {
    let entries = archive.entries();
    let (encrypted_count, compressed_size, size) =
        entries
            .iter()
            .fold((0, 0_u64, 0_u64), |(encrypted, compressed, size), entry| {
                (
                    encrypted + usize::from(entry.is_encrypted()),
                    compressed.saturating_add(entry.compressed_size()),
                    size.saturating_add(entry.uncompressed_size()),
                )
            });
    ArchiveInfo {
        archive: archive.path().display().to_string(),
        channel: p4kx_channels::channel_name(archive.path()),
        build: archive
            .build()
            .map(|build| BuildShape::from(&patches.attach(build))),
        metadata: ArchiveLayoutInfo::of(archive.metadata()),
        entry_count: entries.len(),
        encrypted_count,
        issue_count: archive.issue_count(),
        issues: archive
            .issues()
            .iter()
            .take(MAX_SHOWN_ISSUES)
            .map(|(index, issue)| IssueShape {
                path: archive
                    .entries()
                    .get(*index)
                    .map_or_default(|entry| entry.path().to_owned()),
                issue: issue.to_string(),
            })
            .collect(),
        compressed_size,
        size,
    }
}

/// Returns one page of the Entries of `archive` that `filter` matches, in Archive order.
pub(crate) fn search(
    place: &Place<'_>,
    filter: &str,
    page: Page,
) -> Result<SearchResult, ToolError> {
    let archive = place.archive();
    let filter = Filter::parse(filter)?;
    let mut total = 0;
    let mut selected = Vec::with_capacity(page.limit());
    for entry in archive.entries() {
        if filter.matches(entry.path()) {
            if page.contains(total) {
                selected.push(entry);
            }
            total += 1;
        }
    }
    let next_cursor = page.next_cursor(total)?;
    let entries: Vec<_> = selected
        .iter()
        .map(|entry| EntryShape::at(entry, place.shown(entry.path())))
        .collect();
    let rows = selected
        .into_iter()
        .zip(&entries)
        .map(|(entry, shape)| (entry, &shape.metadata));
    let origins = entry_search_origins::capture(archive, rows, next_cursor.is_some())?;
    Ok(SearchResult {
        origins,
        archive: archive.path().display().to_string(),
        container: place.chain().map(str::to_owned),
        filter: filter.as_str().to_owned(),
        total,
        entries,
        next_cursor,
    })
}

/// Returns one page of the content of `folder`: its subfolders by name ignoring ASCII case,
/// then its Entries in path order. Case and separator kind of `folder` do not matter.
pub(crate) fn list(place: &Place<'_>, folder: &str, page: Page) -> Result<FolderResult, ToolError> {
    let archive = place.archive();
    let tree = place.tree();
    let lookup = FolderLookup::run(tree, folder);
    let (folders, files) = lookup
        .selected
        .map_or_default(|id| (tree.folder(id).children(), tree.files(id)));
    let total = folders.len() + files.len();
    let folder_page = page.slice(folders);
    let file_offset = page.offset().saturating_sub(folders.len()).min(files.len());
    let file_page = files
        .get(file_offset..)
        .unwrap_or_default()
        .iter()
        .take(page.limit() - folder_page.len());
    let next_cursor = page.next_cursor(total)?;
    let mut origins = FolderListingOrigins::observe(archive, tree, &lookup, next_cursor.is_some())?;
    let mut items = Vec::new();
    for &child in folder_page {
        let child = tree.folder(child);
        origins.folder_item(items.len(), tree, child)?;
        items.push(FolderItem::Folder {
            path: place.shown(child.path()),
            entry_count: child.entry_count(),
        });
    }
    for entry in file_page.filter_map(|&id| tree.entry(archive, id)) {
        let shape = EntryShape::at(entry, place.shown(entry.path()));
        origins.entry_item(items.len(), entry, &shape.metadata)?;
        items.push(FolderItem::Entry(shape));
    }
    Ok(FolderResult {
        origins: origins.finish()?,
        archive: archive.path().display().to_string(),
        container: place.chain().map(str::to_owned),
        folder: place.shown(folder.trim_matches(['/', '\\'])),
        total,
        items,
        next_cursor,
    })
}

#[cfg(test)]
#[path = "entries_layout_tests.rs"]
mod tests;
