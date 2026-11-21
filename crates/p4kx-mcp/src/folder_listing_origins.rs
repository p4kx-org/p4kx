// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{
    Archive, Entry, EntryMetadataInfo, EntryTree, FieldOrigins, Folder, FolderId, FolderLookupSite,
    SourceLocation, SourceUseRole, SourceUses,
};

use crate::content_origin::EntryPageSources;
use crate::entry_directory_origins::{
    self, DirectoryOrigins, EntryDirectoryOrigins, EvidenceError, PathUses,
};

pub(crate) struct FolderLookup {
    pub(crate) selected: Option<FolderId>,
    checked: FolderId,
    compared: Vec<FolderId>,
    complete: bool,
}

impl FolderLookup {
    pub(crate) fn run(tree: &EntryTree, path: &str) -> Self {
        let mut lookup = Self {
            selected: None,
            checked: tree.root(),
            compared: Vec::new(),
            complete: true,
        };
        lookup.selected = tree.find_folder_observed(path, |site| match site {
            FolderLookupSite::Children(folder) => lookup.checked = folder,
            FolderLookupSite::Name(folder) => lookup.compared.push(folder),
            _ => lookup.complete = false,
        });
        lookup
    }
}

pub(crate) struct FolderListingOrigins<'archive> {
    archive: &'archive Archive,
    uses: SourceUses,
    nested: Option<NestedDirectory>,
    fields: FieldOrigins,
    page: PathUses,
}

struct NestedDirectory {
    owner: EntryPageSources,
    declaration: PathUses,
}

impl<'archive> FolderListingOrigins<'archive> {
    pub(crate) fn observe(
        archive: &'archive Archive,
        tree: &EntryTree,
        lookup: &FolderLookup,
        has_cursor: bool,
    ) -> Result<Self, EvidenceError> {
        let uses = SourceUses::with_compact_use_refs();
        let nested = archive
            .container_input_origin()
            .map(|origin| -> Result<_, EvidenceError> {
                let owner = EntryPageSources::from_origin_into(Some(&origin), uses.clone())?;
                let declaration =
                    entry_directory_origins::container_directory_paths(archive, &owner)?;
                Ok(NestedDirectory { owner, declaration })
            })
            .transpose()?;
        let mut listing = Self {
            archive,
            uses,
            nested,
            fields: FieldOrigins::new(),
            page: PathUses {
                sources: Vec::new(),
                complete: true,
            },
        };
        listing.navigation(tree, lookup, has_cursor)?;
        Ok(listing)
    }

    fn navigation(
        &mut self,
        tree: &EntryTree,
        lookup: &FolderLookup,
        has_cursor: bool,
    ) -> Result<(), EvidenceError> {
        let local = self.uses.origin(&[], true);
        entry_directory_origins::put(
            &mut self.fields,
            "/structuredContent/archive",
            local.clone(),
        )?;
        let folder = if let Some(nested) = &self.nested {
            let container = nested.owner.path.clone();
            entry_directory_origins::put(
                &mut self.fields,
                "/structuredContent/container",
                container.clone(),
            )?;
            container
        } else {
            local
        };
        entry_directory_origins::put(&mut self.fields, "/structuredContent/folder", folder)?;

        let inspected = tree.folder(lookup.selected.unwrap_or(lookup.checked));
        let mut count = self.directory_paths(inspected.entries())?;
        count.complete &= lookup.complete;
        for &folder in &lookup.compared {
            let compare = self.folder_path(tree, tree.folder(folder), FolderPath::Comparison)?;
            count.sources.extend(compare.sources);
            count.complete &= compare.complete;
        }
        let navigation = self.uses.origin(&count.sources, count.complete);
        entry_directory_origins::put(
            &mut self.fields,
            "/structuredContent/total",
            navigation.clone(),
        )?;
        if has_cursor {
            entry_directory_origins::put(
                &mut self.fields,
                "/structuredContent/next_cursor",
                navigation,
            )?;
        }
        Ok(())
    }

    pub(crate) fn folder_item(
        &mut self,
        ordinal: usize,
        tree: &EntryTree,
        folder: &Folder,
    ) -> Result<(), EvidenceError> {
        let row = format!("/structuredContent/items/{ordinal}");
        let path = self.folder_path(tree, folder, FolderPath::Display)?;
        let count = self.directory_paths(folder.entries())?;
        let path_origin = self.uses.origin(&path.sources, path.complete);
        for field in ["path", "kind"] {
            entry_directory_origins::put(
                &mut self.fields,
                &format!("{row}/{field}"),
                path_origin.clone(),
            )?;
        }
        let count_origin = self.uses.origin(&count.sources, count.complete);
        entry_directory_origins::put(
            &mut self.fields,
            &format!("{row}/entry_count"),
            count_origin,
        )?;
        let mut row_uses = path.sources;
        row_uses.extend(count.sources);
        let complete = path.complete && count.complete;
        entry_directory_origins::put(
            &mut self.fields,
            &row,
            self.uses.origin(&row_uses, complete),
        )?;
        self.page.sources.extend(row_uses);
        self.page.complete &= complete;
        Ok(())
    }

    pub(crate) fn entry_item(
        &mut self,
        ordinal: usize,
        entry: &Entry,
        metadata: &EntryMetadataInfo,
    ) -> Result<(), EvidenceError> {
        let row = format!("/structuredContent/items/{ordinal}");
        let entry = EntryDirectoryOrigins::observe(self.archive, entry, self.uses.clone())?;
        self.page
            .sources
            .extend(entry.write(&mut self.fields, &row, metadata)?);
        self.page.complete &= entry.complete();
        entry_directory_origins::put(
            &mut self.fields,
            &format!("{row}/kind"),
            entry.path_origin().clone(),
        )
    }

    pub(crate) fn finish(mut self) -> Result<DirectoryOrigins, EvidenceError> {
        let items = self.uses.origin(&self.page.sources, self.page.complete);
        entry_directory_origins::put(&mut self.fields, "/structuredContent/items", items)?;
        Ok(DirectoryOrigins {
            field_origins: self.fields,
            provenance: self.uses.freeze()?,
        })
    }

    fn directory_paths(&self, paths: Range<u32>) -> Result<PathUses, EvidenceError> {
        if let Some(nested) = &self.nested {
            let mut observed = nested.declaration.clone();
            observed.sources.push(nested.owner.payload_scope().at(
                SourceLocation::Subresource {
                    namespace: "containerDirectoryTree".into(),
                    indices: vec![u64::from(paths.start), u64::from(paths.end)],
                    field: "paths".into(),
                },
                SourceUseRole::Selector,
            )?);
            Ok(observed)
        } else {
            let observed = self
                .uses
                .observe_root_directory_tree_paths(self.archive, paths)?;
            let complete = observed.is_some();
            Ok(PathUses {
                sources: observed.into_iter().collect(),
                complete,
            })
        }
    }

    fn folder_path(
        &self,
        tree: &EntryTree,
        folder: &Folder,
        site: FolderPath,
    ) -> Result<PathUses, EvidenceError> {
        let mut sources = match (site, &self.nested) {
            (FolderPath::Display, Some(nested)) => nested.owner.path_uses.clone(),
            _ => Vec::new(),
        };
        let paths = folder.entries();
        let first = (!paths.is_empty())
            .then(|| tree.entry(self.archive, paths.start))
            .flatten();
        let Some(first) = first else {
            return Ok(PathUses {
                sources,
                complete: false,
            });
        };
        let entry = EntryPageSources::observe_into(self.archive, first, self.uses.clone())?;
        let end = folder.path().len() as u64;
        let (start, field, role) = match site {
            FolderPath::Display => (0, "folderPath", SourceUseRole::Payload),
            FolderPath::Comparison => (
                end - folder.name().len() as u64,
                "comparedFolderName",
                SourceUseRole::Selector,
            ),
        };
        sources.push(entry.directory_subresource(
            SourceLocation::Subresource {
                namespace: "archiveEntryDecodedPath".into(),
                indices: vec![start, end],
                field: field.into(),
            },
            role,
        )?);
        Ok(PathUses {
            sources,
            complete: entry.complete(),
        })
    }
}

#[derive(Clone, Copy)]
enum FolderPath {
    Display,
    Comparison,
}
