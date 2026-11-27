// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use p4kx_extraction::{Archive, Entry};

use crate::character_path;
use crate::{EntityRig, RigFile};

use super::is_animation_file;

#[derive(Default)]
pub(super) struct Folders<'archive> {
    slots: HashMap<String, Folder<'archive>>,
}

#[derive(Default)]
pub(super) struct Folder<'archive> {
    present: bool,
    pub(super) entries: Vec<&'archive Entry>,
    pub(super) positions: Option<Arc<[usize]>>,
}

impl<'archive> Folders<'archive> {
    pub(super) fn read<E>(
        archive: &'archive Archive,
        rigs: &[EntityRig],
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Self, E> {
        let mut folders = Self::default();
        for rig in rigs {
            check()?;
            for row in rig
                .parameters
                .as_ref()
                .and_then(RigFile::value)
                .into_iter()
                .flatten()
            {
                check()?;
                if row.name.eq_ignore_ascii_case("#filepath")
                    && let Some(path) = requested(&row.path)
                {
                    folders.slots.entry(path).or_default();
                }
            }
        }
        if !folders.slots.is_empty() {
            folders.scan(archive, check)?;
        }
        Ok(folders)
    }

    pub(super) fn get(&mut self, path: &str) -> Option<&mut Folder<'archive>> {
        self.slots
            .get_mut(&requested(path)?)
            .filter(|folder| folder.present)
    }

    fn scan<E>(
        &mut self,
        archive: &'archive Archive,
        check: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), E> {
        for entry in archive.entries() {
            check()?;
            let Some((parent, _)) = entry.path().rsplit_once(['/', '\\']) else {
                continue;
            };
            if !crate::path_match::starts_with_path(parent, "Data/") {
                continue;
            }
            let path = canonical(parent);
            let add = is_animation_file(entry.path());
            for (end, _) in path.match_indices('/').skip(1).chain([(path.len(), "")]) {
                let Some(prefix) = path.get(..end) else {
                    continue;
                };
                if let Some(folder) = self.slots.get_mut(prefix) {
                    folder.present = true;
                    if add {
                        folder.entries.push(entry);
                    }
                }
            }
        }
        Ok(())
    }
}

fn requested(path: &str) -> Option<String> {
    let path = character_path::below_data(path).trim_end_matches(['/', '\\']);
    if path.is_empty() {
        return None;
    }
    Some(canonical(&format!("Data/{path}")))
}

fn canonical(path: &str) -> String {
    let mut path = path.replace('\\', "/");
    path.make_ascii_lowercase();
    path
}
