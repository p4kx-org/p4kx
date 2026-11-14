// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use p4kx_extraction::animation_headers::{AnimationHeaderError, AnimationSourceHeaders};
use p4kx_extraction::{Entry, EntryTree, FolderId};

use crate::character_path;
use crate::entity_rig_catalogue::{RigEntry, read_rig_entry};
use crate::{ArchiveModels, EntityNode, EntityRig, RigFile, discover_entity_rigs};

/// Source-only animation inventory for the resolved placements of an Entity.
#[derive(Debug)]
#[non_exhaustive]
pub struct EntityClipCatalogue {
    /// Ordered independent rig positions and their original parameter links.
    pub rigs: Vec<EntityClipRig>,
    /// One row per selected physical CAF or DBA Entry, in order of first use.
    pub sources: Vec<ClipArchiveSource>,
}

/// One physical Archive animation source selected for a clip inventory.
#[derive(Debug)]
#[non_exhaustive]
pub struct ClipArchiveSource {
    /// Entry position in its containing Archive, also retained for unread payloads.
    pub archive_ordinal: usize,
    /// Physical directory record ordinal when the Archive layout exposes it.
    pub directory_ordinal: Option<u64>,
    /// The read and parse result, whose origin is evidence of successfully read bytes.
    pub file: RigFile<AnimationSourceHeaders, AnimationHeaderError>,
}

/// Authored source links of one rig place in a Loadout.
#[derive(Debug)]
#[non_exhaustive]
pub struct EntityClipRig {
    /// Original rig discovery, including failed reads and unexpanded directives.
    pub rig: EntityRig,
    /// Candidate folder links and direct database selections in authored parameter order.
    pub links: Vec<ClipSourceLink>,
}

/// The result of applying one original parameter directive to Archive paths.
#[derive(Debug)]
#[non_exhaustive]
pub struct ClipSourceLink {
    /// Index into the ordered list of this rig's successfully read parameters.
    pub parameter: usize,
    /// Original directive kind and the available physical links.
    pub selection: ClipSourceSelection,
}

/// Exact file resolution versus recursive archive candidate enumeration.
#[derive(Debug)]
#[non_exhaustive]
pub enum ClipSourceSelection {
    /// Archive candidates below an authored folder. This does not evaluate wildcard directives.
    Folder {
        /// Physical CAF and DBA source rows below the folder, including partial reads.
        sources: Arc<[usize]>,
    },
    /// The authored folder has no matching Archive directory.
    FolderUnavailable,
    /// An authored tracks database named a unique physical Archive Entry.
    Database {
        /// Index into the shared physical source inventory.
        source: usize,
    },
    /// The authored tracks database could not be selected.
    DatabaseUnavailable {
        /// Why no physical source can be selected.
        reason: ClipDatabaseUnavailable,
    },
}

/// Exact lookup disposition for a failed tracks-database selector.
#[derive(Debug)]
#[non_exhaustive]
pub enum ClipDatabaseUnavailable {
    /// The declaration is not a DBA file request.
    UnsupportedPath,
    /// No Archive Entry matches the declaration.
    Missing,
    /// More than one physical Archive Entry matches the declaration.
    Ambiguous,
    /// The Archive lookup itself failed.
    Unreadable {
        /// Original lookup error.
        reason: String,
    },
}

/// Inventories physical CAF and DBA headers without resolving clip-name or wildcard selectors.
///
/// Candidate folders use the same separator and ASCII case rules as normal archive selection.
/// A failed root or individual source read does not stop work for the other placements.
#[must_use]
pub fn discover_entity_clips(models: &ArchiveModels<'_>, root: &EntityNode) -> EntityClipCatalogue {
    let mut inventory = Inventory {
        models,
        physical: PhysicalSources::default(),
        folders: Folders::default(),
    };
    let rigs = discover_entity_rigs(models, root)
        .into_iter()
        .map(|rig| {
            let links = inventory.links(&rig);
            EntityClipRig { rig, links }
        })
        .collect();
    EntityClipCatalogue {
        rigs,
        sources: inventory.physical.rows,
    }
}

struct Inventory<'model, 'archive> {
    models: &'model ArchiveModels<'archive>,
    physical: PhysicalSources,
    folders: Folders,
}

#[derive(Default)]
struct PhysicalSources {
    rows: Vec<ClipArchiveSource>,
    positions: HashMap<usize, usize>,
}

#[derive(Default)]
struct Folders {
    tree: Option<EntryTree>,
    links: HashMap<FolderId, Arc<[usize]>>,
}

impl Inventory<'_, '_> {
    fn links(&mut self, rig: &EntityRig) -> Vec<ClipSourceLink> {
        rig.parameters
            .as_ref()
            .and_then(RigFile::value)
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(parameter, row)| {
                let selection = match row.name.as_str() {
                    "#filepath" => self.folder(&row.path),
                    "$TracksDatabase" => self.database(&row.path),
                    _ => return None,
                };
                Some(ClipSourceLink {
                    parameter,
                    selection,
                })
            })
            .collect()
    }

    fn folder(&mut self, path: &str) -> ClipSourceSelection {
        let folder = character_path::below_data(path).trim_end_matches(['/', '\\']);
        if folder.is_empty() {
            return ClipSourceSelection::FolderUnavailable;
        }
        let archive = self.models.archive;
        let tree = self
            .folders
            .tree
            .get_or_insert_with(|| EntryTree::build(archive));
        let Some(id) = tree.find_folder(&format!("Data/{folder}")) else {
            return ClipSourceSelection::FolderUnavailable;
        };
        if let Some(positions) = self.folders.links.get(&id) {
            return ClipSourceSelection::Folder {
                sources: Arc::clone(positions),
            };
        }
        let sources: Arc<[_]> = tree
            .folder(id)
            .entries()
            .filter_map(|id| tree.entry(archive, id))
            .filter(|entry| is_animation_file(entry.path()))
            .map(|entry| self.physical.resolve(self.models, entry))
            .collect();
        self.folders.links.insert(id, Arc::clone(&sources));
        ClipSourceSelection::Folder { sources }
    }

    fn database(&mut self, path: &str) -> ClipSourceSelection {
        let reason = if has_extension(path, "dba") {
            match self.models.rig_entry(path) {
                RigEntry::Unique(entry) => {
                    return ClipSourceSelection::Database {
                        source: self.physical.resolve(self.models, entry),
                    };
                }
                RigEntry::Missing => ClipDatabaseUnavailable::Missing,
                RigEntry::Ambiguous => ClipDatabaseUnavailable::Ambiguous,
                RigEntry::Unreadable(reason) => ClipDatabaseUnavailable::Unreadable { reason },
            }
        } else {
            ClipDatabaseUnavailable::UnsupportedPath
        };
        ClipSourceSelection::DatabaseUnavailable { reason }
    }
}

impl PhysicalSources {
    fn resolve(&mut self, models: &ArchiveModels<'_>, entry: &Entry) -> usize {
        *self.positions.entry(entry.index()).or_insert_with(|| {
            let position = self.rows.len();
            let read = read_rig_entry(models, entry, |bytes| {
                AnimationSourceHeaders::read(entry.path(), bytes)
            });
            self.rows.push(ClipArchiveSource {
                archive_ordinal: entry.index(),
                directory_ordinal: entry.metadata().original_directory_ordinal(),
                file: RigFile {
                    path: entry.path().to_owned(),
                    read,
                },
            });
            position
        })
    }
}

fn is_animation_file(path: &str) -> bool {
    has_extension(path, "caf") || has_extension(path, "dba")
}

fn has_extension(path: &str, wanted: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case(wanted))
}

#[cfg(test)]
#[path = "entity_clip_catalogue_tests.rs"]
mod tests;
