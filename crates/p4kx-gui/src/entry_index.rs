// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod backing;
mod containers;
mod file;

pub use backing::NoEntries;
pub(crate) use containers::is_container;

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_channels::ChannelKind;
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, Entry, EntryTree, Filter, Folder, FolderId,
    LocalizationEntry, PathIndex, PathKeys, TextureGroups,
};
use rayon::prelude::*;

use self::backing::Backing;
use crate::TextLanguage;
use crate::entry_groups::EntryGroups;
use crate::forge_cache::ForgeCache;
use crate::sort::{Sort, SortDirection, SortOrders, file_name};
use crate::text_cache::TextCache;

/// An opened Archive with its [`EntryTree`] and its Entries pre-sorted by every column.
///
/// Entries are addressed by their id in the tree, so the Entries of a folder and everything
/// below it form one contiguous id range.
#[derive(Debug)]
pub struct EntryIndex {
    backing: Backing,
    watch: ArchiveWatch,
    tree: EntryTree,
    orders: SortOrders,
    containers: Vec<(FolderId, u32)>,
    path_bytes: u64,
    groups: EntryGroups,
    id_of: Vec<u32>,
    dataforge: Option<usize>,
    forge: ForgeCache,
    paths: OnceLock<PathKeys>,
    text_languages: Vec<TextLanguage>,
    texts: TextCache,
    channel: ChannelKind,
}

impl EntryIndex {
    /// Builds the index of `archive`; this sorts every Entry several times and belongs on a
    /// background thread for large Archives.
    #[must_use]
    pub fn build(archive: Archive) -> Self {
        let entries = archive.entries();
        let (texture_groups, tree) = rayon::join(
            || TextureGroups::build(&archive),
            || EntryTree::build(&archive),
        );
        let sorted: Vec<&Entry> = (0..tree.len())
            .filter_map(|id| tree.entry(&archive, u32::try_from(id).ok()?))
            .collect();
        let mut id_of = vec![0; entries.len()];
        for id in 0..tree.len() {
            let position = u32::try_from(id)
                .ok()
                .and_then(|id| tree.position(id))
                .and_then(|position| id_of.get_mut(position));
            if let Some(slot) = position {
                *slot = u32::try_from(id).unwrap_or(u32::MAX);
            }
        }
        let groups = EntryGroups::new(&texture_groups, &id_of, tree.len());
        let orders = SortOrders::build(&sorted, |id| groups.total_size(id));
        let containers = containers::build(&sorted, &tree);
        let path_bytes = sorted
            .par_iter()
            .map(|entry| entry.path().len() as u64)
            .sum();
        let dataforge = DataForgeEntry::locate(&archive)
            .and_then(|found| entries.iter().position(|entry| std::ptr::eq(entry, found)));
        let text_languages = LocalizationEntry::languages(&archive);
        Self {
            channel: ChannelKind::of_archive(archive.path()),
            watch: ArchiveWatch::new(&archive),
            backing: Backing::Archive(Box::new(archive)),
            tree,
            orders,
            containers,
            path_bytes,
            groups,
            id_of,
            dataforge,
            forge: ForgeCache::default(),
            paths: OnceLock::new(),
            text_languages,
            texts: TextCache::default(),
        }
    }

    /// Returns the kind of the Channel the Archive lives in, told when the index was built.
    #[must_use]
    pub const fn channel(&self) -> ChannelKind {
        self.channel
    }

    /// Returns the ids of the containers directly in `folder`, by name ignoring case.
    pub fn containers_in(&self, folder: FolderId) -> impl ExactSizeIterator<Item = u32> + '_ {
        containers::directly_in(&self.containers, folder)
            .iter()
            .map(|&(_, id)| id)
    }

    /// Returns whether `folder` shows anything below it in the path tree: subfolders or
    /// containers.
    #[must_use]
    pub fn has_children(&self, folder: FolderId) -> bool {
        !self.folder(folder).children().is_empty() || self.containers_in(folder).len() > 0
    }

    /// Returns roughly how many bytes the Entries and this index hold in memory, without the
    /// data of a decoded container.
    pub(crate) const fn footprint(&self) -> u64 {
        let per_entry = size_of::<Entry>() + size_of::<u32>() + containers::INDEX_BYTES_PER_ENTRY;
        (self.len() as u64)
            .saturating_mul(per_entry as u64)
            .saturating_add(self.path_bytes)
    }

    /// Returns the name DataForge is shown under: the path of its Entry in the Archive, or the
    /// file name of a DataForge file; `None` when there is no DataForge.
    #[must_use]
    pub fn dataforge_name(&self) -> Option<String> {
        self.forge_source().name()
    }

    /// Returns the size of DataForge after decompression: that of its Entry, or of the file; `0`
    /// when there is no DataForge.
    #[must_use]
    pub fn dataforge_size(&self) -> u64 {
        self.backing
            .file_len()
            .unwrap_or_else(|| self.dataforge_entry().map_or(0, Entry::uncompressed_size))
    }

    /// Returns the Entry DataForge is read from, `Data\Game2.dcb` or `Data\Game.dcb`, or `None`
    /// when the Archive holds neither.
    #[must_use]
    pub fn dataforge_entry(&self) -> Option<&Entry> {
        let position = self.dataforge?;
        self.archive()?.entries().get(position)
    }

    /// Returns the one DataForge cache of the Archive.
    pub(crate) const fn forge(&self) -> &ForgeCache {
        &self.forge
    }

    /// Returns the lookup of Entries by path over the Archive, or `None` for a DataForge file;
    /// the path hashes are built on the first call and shared by every later one.
    pub(crate) fn path_index(&self) -> Option<PathIndex<'_>> {
        let archive = self.archive()?;
        Some(
            self.paths
                .get_or_init(|| PathKeys::new(archive))
                .index(archive),
        )
    }

    /// Returns the one cache of the localisation texts of the Archive.
    pub(crate) const fn texts(&self) -> &TextCache {
        &self.texts
    }

    /// Returns the languages the Archive holds a `global.ini` for, in the order of
    /// [`TextLanguage::ALL`].
    #[must_use]
    pub fn text_languages(&self) -> &[TextLanguage] {
        &self.text_languages
    }

    /// Returns how often a `global.ini` was read from the Archive; the Record previews share
    /// each read.
    #[must_use]
    pub fn text_loads(&self) -> usize {
        self.texts.loads()
    }

    /// Returns the folder tree of the Archive.
    #[must_use]
    pub const fn tree(&self) -> &EntryTree {
        &self.tree
    }

    /// Returns the watch that notices when the Archive changes on disk.
    #[must_use]
    pub const fn watch(&self) -> &ArchiveWatch {
        &self.watch
    }

    /// Returns the number of Entries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.tree.len()
    }

    /// Returns whether the Archive has no Entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    /// Returns the parts and alpha tail Entries of the split texture whose `.dds` header has id
    /// `id`; empty for any other Entry.
    #[must_use]
    pub fn texture_parts(&self, id: u32) -> &[u32] {
        self.groups.parts_of(id)
    }

    /// Returns every Entry of the split texture `id` belongs to, its header first, or `None`
    /// when `id` belongs to none.
    #[must_use]
    pub fn texture_members(&self, id: u32) -> Option<&[u32]> {
        self.groups.members(id)
    }

    /// Returns the id of the header of the split texture `id` belongs to, or `id` itself when it
    /// belongs to none.
    #[must_use]
    pub fn texture_header(&self, id: u32) -> u32 {
        self.groups
            .members(id)
            .and_then(<[u32]>::first)
            .copied()
            .unwrap_or(id)
    }

    /// Returns whether `id` is a part or alpha tail Entry of a split texture, which the listing
    /// folds into the row of the texture's header.
    #[must_use]
    pub fn is_texture_part(&self, id: u32) -> bool {
        self.groups.is_part(id)
    }

    /// Returns the size after decompression of the whole split texture whose header has id
    /// `id`, or `None` when `id` is no such header.
    #[must_use]
    pub fn texture_size(&self, id: u32) -> Option<u64> {
        self.groups.total_size(id)
    }

    /// Returns the number of Entries folded into the row of a texture header.
    #[must_use]
    pub const fn folded_len(&self) -> usize {
        self.groups.part_count()
    }

    /// Returns the id of the Entry at `path`, ignoring ASCII case and the separator kind, or
    /// `None` when there is none.
    #[must_use]
    pub fn id_of_path(&self, path: &str) -> Option<u32> {
        self.id_at_position(self.path_index()?.position(path)?)
    }

    pub(crate) fn id_of_entry(&self, entry: &Entry) -> Option<u32> {
        let position = entry.index();
        let archive_entry = self.archive()?.entries().get(position)?;
        if !std::ptr::eq(archive_entry, entry) {
            return None;
        }
        self.id_at_position(position)
    }

    fn id_at_position(&self, position: usize) -> Option<u32> {
        self.id_of
            .get(position)
            .copied()
            .filter(|&id| self.tree.position(id) == Some(position))
    }

    /// Returns the Entry with id `id`, or `None` when there is none.
    #[must_use]
    pub fn entry(&self, id: u32) -> Option<&Entry> {
        self.tree.entry(self.archive()?, id)
    }

    /// Returns the file name of the Entry with id `id`, empty when there is none.
    #[must_use]
    pub fn name(&self, id: u32) -> &str {
        self.entry(id).map_or("", |entry| file_name(entry.path()))
    }

    /// Returns the root folder, which holds every Entry.
    #[must_use]
    pub const fn root(&self) -> FolderId {
        self.tree.root()
    }

    /// Returns the folder `id`; an id of another index yields an empty folder.
    #[must_use]
    pub fn folder(&self, id: FolderId) -> &Folder {
        self.tree.folder(id)
    }

    /// Returns the folder at `path`, or `None` when there is none; the empty path is the root.
    /// ASCII case and separator kind do not matter.
    #[must_use]
    pub fn find_folder(&self, path: &str) -> Option<FolderId> {
        self.tree.find_folder(path)
    }

    /// Returns the ids of the Entries in `folder` and below, sorted by `sort`; the parts of a
    /// split texture are folded into its header and left out.
    #[must_use]
    pub fn entries_in(&self, folder: FolderId, sort: Sort) -> Vec<u32> {
        let range = self.folder(folder).entries();
        self.sorted_where(sort, |id| range.contains(&id))
    }

    /// Returns the ids of the Entries marked in `matches`, which is indexed by id, sorted by
    /// `sort`; the parts of a split texture are folded into its header and left out.
    #[must_use]
    pub fn entries_matching(&self, matches: &[bool], sort: Sort) -> Vec<u32> {
        self.sorted_where(sort, |id| {
            matches.get(id as usize).copied().unwrap_or(false)
        })
    }

    /// Returns, indexed by id, whether the path of each Entry matches `filter`, or `None` once
    /// `cancelled` is set; all Entries of a split texture match when one of them does.
    #[must_use]
    pub fn matching(&self, filter: &Filter, cancelled: &AtomicBool) -> Option<Vec<bool>> {
        let mut matches: Vec<bool> = (0..self.tree.len())
            .into_par_iter()
            .map(|id| {
                !cancelled.load(Ordering::Relaxed)
                    && u32::try_from(id)
                        .ok()
                        .and_then(|id| self.entry(id))
                        .is_some_and(|entry| filter.matches(entry.path()))
            })
            .collect();
        self.groups.complete(&mut matches);
        (!cancelled.load(Ordering::Relaxed)).then_some(matches)
    }

    /// Fills `out` with the ids [`EntryIndex::entries_in`] returns, reusing its allocation.
    pub(crate) fn entries_in_into(&self, folder: FolderId, sort: Sort, out: &mut Vec<u32>) {
        let range = self.folder(folder).entries();
        self.sorted_where_into(sort, |id| range.contains(&id), out);
    }

    /// Fills `out` with the ids [`EntryIndex::entries_matching`] returns, reusing its allocation.
    pub(crate) fn entries_matching_into(&self, matches: &[bool], sort: Sort, out: &mut Vec<u32>) {
        self.sorted_where_into(
            sort,
            |id| matches.get(id as usize).copied().unwrap_or(false),
            out,
        );
    }

    fn sorted_where(&self, sort: Sort, keep: impl Fn(u32) -> bool) -> Vec<u32> {
        let mut out = Vec::new();
        self.sorted_where_into(sort, keep, &mut out);
        out
    }

    fn sorted_where_into(&self, sort: Sort, keep: impl Fn(u32) -> bool, out: &mut Vec<u32>) {
        out.clear();
        let ascending = self.orders.ascending(sort.column).iter();
        let keep = |id: &&u32| keep(**id) && !self.groups.is_part(**id);
        match sort.direction {
            SortDirection::Ascending => out.extend(ascending.filter(keep).copied()),
            SortDirection::Descending => out.extend(ascending.rev().filter(keep).copied()),
        }
    }
}
