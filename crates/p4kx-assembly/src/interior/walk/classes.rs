// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_dataforge::Guid;

use super::{Todo, Walker};
use crate::{Geometry, IssueKind, PaletteRef};

impl Walker<'_> {
    /// Returns the geometry the class Record `text` names, looked up once per Record.
    pub(super) fn class_geometry(&mut self, text: &str) -> Option<Arc<Geometry>> {
        let lookup = self.classes?;
        if let Some(found) = self.class_geometry.get(text) {
            return found.clone();
        }
        let found = text
            .parse::<Guid>()
            .ok()
            .and_then(|guid| lookup.geometry(guid));
        self.class_geometry.insert(text.to_owned(), found.clone());
        found
    }

    /// Returns the palette at the record path `path` of an `IncludedObjects` table, looked up
    /// once per path; notes why a path names none.
    pub(super) fn palette_at(&mut self, todo: &Todo, file: &str, path: &str) -> Option<PaletteRef> {
        let lookup = self.classes?;
        if let Some(found) = self.palette_paths.get(path) {
            return found.clone();
        }
        let found = match lookup.palettes().guid_at_observed(path) {
            Ok(read) => Some(PaletteRef {
                guid: read.value,
                child_path: String::new(),
                selector: crate::geometry::PaletteReferenceSource::IncludedPath(read.record),
            }),
            Err(failure) => {
                let reason = format!("the tint palette {path} cannot be used: {failure}");
                self.note(&todo.path, file, IssueKind::Palette { reason });
                None
            }
        };
        self.palette_paths.insert(path.to_owned(), found.clone());
        found
    }
}
