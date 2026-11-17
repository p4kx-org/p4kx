// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use super::{PlacementRows, System, WalkPath};
use crate::starmap_containers::instance::ContainerFile;
use crate::starmap_containers::unresolved::UnresolvedOccurrence;
use crate::starmap_parse::Child;
use crate::{CatalogError, ExportAccountingError};

impl System<'_> {
    pub(super) fn record_unresolved_container(
        &mut self,
        child: &Child,
        parent: &WalkPath,
        placed: Option<&WalkPath>,
        file: &ContainerFile,
    ) -> Result<(), CatalogError> {
        let Some(declaration) = &child.unresolved_external else {
            return Ok(());
        };
        let selector = child
            .external
            .as_ref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let path = placed.map_or_else(
            || Cow::Owned(parent.child(child, None, parent.transform)),
            Cow::Borrowed,
        );
        let route = path
            .route
            .as_deref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let occurrence = UnresolvedOccurrence {
            root_entry: self.root_entry,
            root_inputs: &self.root,
            route,
            parent_route: path.parent_instance.as_deref(),
            selector,
            declaration,
            world: placed.map(|path| path.transform),
        };
        match &mut self.rows {
            PlacementRows::Spool { unresolved, .. } => unresolved
                .append(
                    &occurrence.project(self.archive, self.index, file, self.capture)?,
                    self.cancel,
                )
                .map_err(Into::into),
            #[cfg(test)]
            PlacementRows::Typed => {
                self.read.unresolved.push(occurrence.project(
                    self.archive,
                    self.index,
                    file,
                    self.capture,
                )?);
                Ok(())
            }
            PlacementRows::Harvest(_) => Ok(()),
        }
    }
}
