// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;

use p4kx_extraction::animation_headers::{AnimationHeaderError, AnimationSourceHeaders};
use p4kx_extraction::{Entry, NamedCafSelector};

use crate::entity_rig_catalogue::{RigEntry, discover_entity_rigs_with, read_rig_entry};
use crate::{ArchiveModels, EntityNode, EntityRig, RigFile, RigParameter};

#[path = "entity_clip_folders.rs"]
mod folders;

use folders::Folders;

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
    /// Candidate folders and exact database or named CAF links in authored parameter order.
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
    /// A concrete CAF alias and its physical selection, even when the file could not be located.
    NamedCaf {
        /// Earlier folder parameter used to resolve a relative path within this `AnimationList`.
        filepath_parameter: Option<usize>,
        /// Selected physical source or the reason why none has been selected.
        target: NamedCafTarget,
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

/// The available physical file of a named CAF alias.
#[derive(Debug)]
#[non_exhaustive]
pub enum NamedCafTarget {
    /// Index into the shared physical source inventory.
    File(usize),
    /// A relative alias has no preceding folder within its `AnimationList`.
    MissingScope,
    /// Path spelling cannot be resolved to an eligible Archive path.
    InvalidPath,
    /// No Archive Entry matches the selector.
    Missing,
    /// More than one physical Archive Entry matches the selector.
    Ambiguous,
    /// Archive lookup failed before physical identity could be determined.
    Unreadable {
        /// The lookup diagnostic.
        reason: String,
    },
}

/// Inventories physical CAF and DBA headers without expanding wildcard selectors.
///
/// Candidate folders use the same separator and ASCII case rules as normal archive selection.
/// A failed root or individual source read does not stop work for the other placements.
#[must_use]
pub fn discover_entity_clips(models: &ArchiveModels<'_>, root: &EntityNode) -> EntityClipCatalogue {
    match discover_entity_clips_with(models, root, &mut || Ok::<_, Infallible>(())) {
        Ok(catalogue) => catalogue,
        Err(never) => match never {},
    }
}

/// Inventories entity clips, polling the caller during source discovery and header reading.
///
/// # Errors
/// Returns the exact checkpoint failure. A failed Archive source remains an inventory row.
pub fn discover_entity_clips_with<E>(
    models: &ArchiveModels<'_>,
    root: &EntityNode,
    check: &mut impl FnMut() -> Result<(), E>,
) -> Result<EntityClipCatalogue, E> {
    let rigs = discover_entity_rigs_with(models, root, check)?;
    let mut inventory = Inventory {
        models,
        physical: PhysicalSources::default(),
        folders: Folders::read(models.archive, &rigs, check)?,
    };
    let rigs = rigs
        .into_iter()
        .map(|rig| {
            let links = inventory.links(&rig, check)?;
            Ok(EntityClipRig { rig, links })
        })
        .collect::<Result<_, E>>()?;
    check()?;
    Ok(EntityClipCatalogue {
        rigs,
        sources: inventory.physical.rows,
    })
}

struct Inventory<'model, 'archive> {
    models: &'model ArchiveModels<'archive>,
    physical: PhysicalSources,
    folders: Folders<'archive>,
}

#[derive(Default)]
struct PhysicalSources {
    rows: Vec<ClipArchiveSource>,
    positions: HashMap<usize, usize>,
}

impl Inventory<'_, '_> {
    fn links<E>(
        &mut self,
        rig: &EntityRig,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<ClipSourceLink>, E> {
        let mut folder = None;
        let mut links = Vec::new();
        let Some(parameters) = rig.parameters.as_ref().and_then(RigFile::value) else {
            return Ok(links);
        };
        for (parameter, row) in parameters.iter().enumerate() {
            check()?;
            let selection = if row.name.eq_ignore_ascii_case("#filepath") {
                folder = Some((parameter, row));
                self.folder(&row.path, check)?
            } else if row.name.eq_ignore_ascii_case("$TracksDatabase") {
                self.database(&row.path, check)?
            } else if let Some(caf) = NamedCafSelector::new(&row.name, &row.path) {
                self.named_caf(
                    caf,
                    folder.filter(|(_, scope)| scope.list == row.list),
                    check,
                )?
            } else {
                continue;
            };
            links.push(ClipSourceLink {
                parameter,
                selection,
            });
        }
        Ok(links)
    }

    fn named_caf<E>(
        &mut self,
        selector: NamedCafSelector<'_>,
        folder: Option<(usize, &RigParameter)>,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<ClipSourceSelection, E> {
        let scope = folder.filter(|_| selector.uses_scope());
        let target = if selector.uses_scope() && scope.is_none() {
            NamedCafTarget::MissingScope
        } else if let Some(path) = selector.archive_path(scope.map(|(_, row)| row.path.as_str())) {
            match self.models.exact_rig_entry(&path, check)? {
                RigEntry::Unique(entry) => {
                    NamedCafTarget::File(self.physical.resolve(self.models, entry, check)?)
                }
                RigEntry::Missing => NamedCafTarget::Missing,
                RigEntry::Ambiguous => NamedCafTarget::Ambiguous,
                RigEntry::Unreadable(reason) => NamedCafTarget::Unreadable { reason },
            }
        } else {
            NamedCafTarget::InvalidPath
        };
        Ok(ClipSourceSelection::NamedCaf {
            filepath_parameter: scope.map(|(parameter, _)| parameter),
            target,
        })
    }

    fn folder<E>(
        &mut self,
        path: &str,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<ClipSourceSelection, E> {
        let Some(folder) = self.folders.get(path) else {
            return Ok(ClipSourceSelection::FolderUnavailable);
        };
        let sources = if let Some(positions) = &folder.positions {
            Arc::clone(positions)
        } else {
            let positions: Arc<[_]> = folder
                .entries
                .iter()
                .map(|entry| self.physical.resolve(self.models, entry, check))
                .collect::<Result<_, E>>()?;
            folder.positions = Some(Arc::clone(&positions));
            positions
        };
        Ok(ClipSourceSelection::Folder { sources })
    }

    fn database<E>(
        &mut self,
        path: &str,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<ClipSourceSelection, E> {
        let reason = if has_extension(path, "dba") {
            match self.models.rig_entry(path, check)? {
                RigEntry::Unique(entry) => {
                    return Ok(ClipSourceSelection::Database {
                        source: self.physical.resolve(self.models, entry, check)?,
                    });
                }
                RigEntry::Missing => ClipDatabaseUnavailable::Missing,
                RigEntry::Ambiguous => ClipDatabaseUnavailable::Ambiguous,
                RigEntry::Unreadable(reason) => ClipDatabaseUnavailable::Unreadable { reason },
            }
        } else {
            ClipDatabaseUnavailable::UnsupportedPath
        };
        Ok(ClipSourceSelection::DatabaseUnavailable { reason })
    }
}

impl PhysicalSources {
    fn resolve<E>(
        &mut self,
        models: &ArchiveModels<'_>,
        entry: &Entry,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<usize, E> {
        check()?;
        let slot = match self.positions.entry(entry.index()) {
            std::collections::hash_map::Entry::Occupied(slot) => return Ok(*slot.get()),
            std::collections::hash_map::Entry::Vacant(slot) => slot,
        };
        let read = read_rig_entry(
            models,
            entry,
            |bytes, check| AnimationSourceHeaders::read_with(entry.path(), bytes, check),
            check,
        )?;
        let position = self.rows.len();
        self.rows.push(ClipArchiveSource {
            archive_ordinal: entry.index(),
            directory_ordinal: entry.metadata().original_directory_ordinal(),
            file: RigFile {
                path: entry.path().to_owned(),
                read,
            },
        });
        slot.insert(position);
        Ok(position)
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
