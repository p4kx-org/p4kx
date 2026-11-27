// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use super::{PlacementRows, System, WalkPath};
use crate::catalog_queries::starmap::ContainerPlacementEvent;
use crate::starmap_containers::instance::ContainerFile;
use crate::starmap_containers::unresolved::{ContainerTargetReadStatus, UnresolvedOccurrence};
use crate::starmap_parse::Body;
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
        if matches!(
            &self.rows,
            PlacementRows::Harvest(_) | PlacementRows::Object(_) | PlacementRows::ShopNames(_)
        ) {
            return Ok(());
        }
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
        if !self.rows.includes_container(Some(route)) {
            return Ok(());
        }
        let read_status = self.unresolved_target_status(selector.as_str())?;
        let occurrence = UnresolvedOccurrence {
            root_entry: self.root_entry,
            root_inputs: &self.root,
            route,
            parent_route: path.parent_instance.as_deref(),
            selector,
            declaration,
            world: placed.map(|path| path.transform),
            read_status,
        };
        let row = occurrence.project(self.archive, self.index, file, self.capture)?;
        match &mut self.rows {
            PlacementRows::Spool { unresolved, .. } => {
                unresolved.append(&row, self.cancel).map_err(Into::into)
            }
            #[cfg(test)]
            PlacementRows::Typed => {
                self.read.unresolved.push(row);
                Ok(())
            }
            PlacementRows::Selected(rows) => {
                rows.unresolved.push(row);
                Ok(())
            }
            PlacementRows::ContainerVisitor(visit) => {
                self.read.visited_container_rows.record_unresolved_link()?;
                visit(ContainerPlacementEvent::UnresolvedLink(&row))
            }
            PlacementRows::Harvest(_)
            | PlacementRows::OpenedContainers
            | PlacementRows::Object(_)
            | PlacementRows::ShopNames(_) => Ok(()),
        }
    }

    fn unresolved_target_status(
        &self,
        selector: &str,
    ) -> Result<ContainerTargetReadStatus, CatalogError> {
        match self.index.candidates(selector) {
            [target] => match self.bodies.get(target) {
                Some(Body::Unreadable(_)) => Ok(ContainerTargetReadStatus::Unreadable),
                Some(Body::Unparsed) => Ok(ContainerTargetReadStatus::Unparsed),
                _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
            },
            _ => Ok(ContainerTargetReadStatus::NotAttempted),
        }
    }
}
