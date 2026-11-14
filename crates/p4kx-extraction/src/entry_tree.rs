// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;
use std::ops::Range;

use p4kx_archive::{Archive, Entry};
use rayon::slice::ParallelSliceMut;

/// Identifies a folder of an [`EntryTree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FolderId(usize);

impl FolderId {
    /// The root folder, which holds every Entry.
    pub const ROOT: Self = Self(0);
}

/// One folder of an [`EntryTree`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    path: String,
    name_start: usize,
    children: Vec<FolderId>,
    entries: Range<u32>,
    files: Range<usize>,
}

static NO_FOLDER: Folder = Folder {
    path: String::new(),
    name_start: 0,
    children: Vec::new(),
    entries: 0..0,
    files: 0..0,
};

impl Folder {
    /// Returns the folder name, empty for the root.
    #[must_use]
    pub fn name(&self) -> &str {
        self.path.get(self.name_start..).unwrap_or_default()
    }

    /// Returns the path of the folder in the Archive as its first Entry spells it, empty for the
    /// root.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the subfolders, ordered by name ignoring ASCII case.
    #[must_use]
    pub fn children(&self) -> &[FolderId] {
        &self.children
    }

    /// Returns the number of Entries in this folder and every folder below it.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns the ids of the Entries in this folder and below, one contiguous range.
    #[must_use]
    pub fn entries(&self) -> Range<u32> {
        self.entries.clone()
    }
}

/// One consulted site in a folder lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FolderLookupSite {
    /// The folder whose child names are searched.
    Children(FolderId),
    /// The folder whose original name is compared.
    Name(FolderId),
}

/// The folders of an Archive, built once after opening it.
///
/// Folder names are compared ignoring ASCII case with `/` and `\` alike, so spellings that
/// differ only so are one folder (ADR-0023). Entries are addressed by id, their position in
/// path order: by path ignoring ASCII case with `\` as `/`, then byte by byte. The Entries of a
/// folder and everything below it form one contiguous id range.
#[derive(Debug, Default)]
pub struct EntryTree {
    by_path: Vec<u32>,
    folders: Vec<Folder>,
    files: Vec<u32>,
}

impl EntryTree {
    /// Builds the tree of `archive`; this sorts every Entry and belongs on a background thread
    /// for large Archives.
    #[must_use]
    pub fn build(archive: &Archive) -> Self {
        let entries = archive.entries();
        let mut by_path: Vec<u32> = (0..entries.len())
            .filter_map(|position| u32::try_from(position).ok())
            .collect();
        let path = |position: u32| entries.get(position as usize).map_or("", Entry::path);
        by_path.par_sort_unstable_by(|&a, &b| compare_paths(path(a), path(b)));
        let mut builder = Builder::default();
        for (id, &position) in by_path.iter().enumerate() {
            builder.add(path(position), u32::try_from(id).unwrap_or(u32::MAX));
        }
        let (folders, files) = builder.finish();
        Self {
            by_path,
            folders,
            files,
        }
    }

    /// Returns the number of Entries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.by_path.len()
    }

    /// Returns whether the Archive has no Entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }

    /// Returns the position in `archive` of the Entry with id `id`.
    #[must_use]
    pub fn position(&self, id: u32) -> Option<usize> {
        self.by_path
            .get(id as usize)
            .map(|&position| position as usize)
    }

    /// Returns the Entry with id `id` of `archive`, the Archive the tree was built from.
    #[must_use]
    pub fn entry<'a>(&self, archive: &'a Archive, id: u32) -> Option<&'a Entry> {
        archive.entries().get(self.position(id)?)
    }

    /// Returns the root folder.
    #[must_use]
    pub const fn root(&self) -> FolderId {
        FolderId::ROOT
    }

    /// Returns the folder `id`; an id of another tree yields an empty folder.
    #[must_use]
    pub fn folder(&self, id: FolderId) -> &Folder {
        self.folders.get(id.0).unwrap_or(&NO_FOLDER)
    }

    /// Returns the ids of the Entries directly in `folder`, in path order.
    #[must_use]
    pub fn files(&self, folder: FolderId) -> &[u32] {
        self.files
            .get(self.folder(folder).files.clone())
            .unwrap_or_default()
    }

    /// Returns the folder at `path`, ignoring ASCII case, separator kind and separators at
    /// either end; the empty path is the root.
    #[must_use]
    pub fn find_folder(&self, path: &str) -> Option<FolderId> {
        self.find_folder_observed(path, |_| {})
    }

    /// Finds a folder and reports the child lists and names consulted during the lookup.
    #[must_use]
    pub fn find_folder_observed(
        &self,
        path: &str,
        mut observe: impl FnMut(FolderLookupSite),
    ) -> Option<FolderId> {
        let path = path.trim_matches(is_separator);
        if path.is_empty() {
            return Some(self.root());
        }
        path.split(is_separator)
            .try_fold(self.root(), |folder, name| {
                observe(FolderLookupSite::Children(folder));
                let children = self.folder(folder).children();
                children
                    .binary_search_by(|&child| {
                        observe(FolderLookupSite::Name(child));
                        compare_names(self.folder(child).name(), name)
                    })
                    .ok()
                    .and_then(|index| children.get(index).copied())
            })
    }
}

/// Builds the folders from Entry paths in path order.
#[derive(Debug)]
struct Builder {
    folders: Vec<Folder>,
    open: Vec<usize>,
    parents: Vec<usize>,
    next_id: u32,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            folders: vec![NO_FOLDER.clone()],
            open: vec![0],
            parents: Vec::new(),
            next_id: 0,
        }
    }
}

impl Builder {
    fn add(&mut self, path: &str, id: u32) {
        if let Some(folder_end) = path.rfind(is_separator) {
            let parts = path.get(..folder_end).unwrap_or_default();
            let mut start = 0;
            let mut depth = 0;
            for part in parts.split(is_separator) {
                let end = start + part.len();
                depth += 1;
                let same = self
                    .open
                    .get(depth)
                    .and_then(|&folder| self.folders.get(folder))
                    .is_some_and(|folder| folder.name().eq_ignore_ascii_case(part));
                if !same {
                    self.close(depth, id);
                    self.open_child(path.get(..end).unwrap_or_default(), start, id);
                }
                start = end + 1;
            }
            self.close(depth + 1, id);
        } else {
            self.close(1, id);
        }
        self.parents
            .push(self.open.last().copied().unwrap_or_default());
        self.next_id = id.saturating_add(1);
    }

    fn close(&mut self, keep: usize, end: u32) {
        while self.open.len() > keep {
            if let Some(folder) = self
                .open
                .pop()
                .and_then(|folder| self.folders.get_mut(folder))
            {
                folder.entries.end = end;
            }
        }
    }

    fn open_child(&mut self, path: &str, name_start: usize, first: u32) {
        let id = self.folders.len();
        if let Some(parent) = self
            .open
            .last()
            .and_then(|&parent| self.folders.get_mut(parent))
        {
            parent.children.push(FolderId(id));
        }
        self.folders.push(Folder {
            path: path.to_owned(),
            name_start,
            children: Vec::new(),
            entries: first..first,
            files: 0..0,
        });
        self.open.push(id);
    }

    fn finish(mut self) -> (Vec<Folder>, Vec<u32>) {
        self.close(0, self.next_id);
        let mut counts = vec![0_usize; self.folders.len()];
        for &parent in &self.parents {
            if let Some(count) = counts.get_mut(parent) {
                *count += 1;
            }
        }
        let mut start = 0;
        for (folder, count) in self.folders.iter_mut().zip(&counts) {
            folder.files = start..start + count;
            start += count;
        }
        let mut next: Vec<usize> = self
            .folders
            .iter()
            .map(|folder| folder.files.start)
            .collect();
        let mut files = vec![0_u32; self.parents.len()];
        for (id, &parent) in self.parents.iter().enumerate() {
            if let Some(at) = next.get_mut(parent)
                && let Some(slot) = files.get_mut(*at)
            {
                *slot = u32::try_from(id).unwrap_or(u32::MAX);
                *at += 1;
            }
        }
        let names: Vec<String> = self
            .folders
            .iter()
            .map(|folder| folder.name().to_owned())
            .collect();
        let name = |id: &FolderId| names.get(id.0).map_or("", String::as_str);
        for folder in &mut self.folders {
            folder
                .children
                .sort_unstable_by(|a, b| compare_names(name(a), name(b)));
        }
        (self.folders, files)
    }
}

const fn is_separator(c: char) -> bool {
    c == '/' || c == '\\'
}

fn compare_names(a: &str, b: &str) -> Ordering {
    let key = |byte: u8| byte.to_ascii_lowercase();
    a.bytes().map(key).cmp(b.bytes().map(key))
}

fn compare_paths(a: &str, b: &str) -> Ordering {
    let key = |byte: u8| {
        if byte == b'\\' {
            b'/'
        } else {
            byte.to_ascii_lowercase()
        }
    };
    a.bytes()
        .map(key)
        .cmp(b.bytes().map(key))
        .then_with(|| a.cmp(b))
}
