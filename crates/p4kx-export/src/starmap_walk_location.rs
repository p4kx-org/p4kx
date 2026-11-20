// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use super::{PlacementRows, System, WalkPath};
use crate::starmap_containers::instance::{ContainerFile, ContainerOccurrence};
use crate::starmap_containers::location_instance::LocationOccurrence;
use crate::starmap_parse::Child;
use crate::starmap_xml::AuthoredPose;
use crate::{CatalogError, ExportAccountingError};

impl System<'_> {
    pub(super) fn record_instance(
        &mut self,
        path: &WalkPath,
        file: &ContainerFile,
    ) -> Result<(), CatalogError> {
        if !self.rows.includes_container(path.route.as_deref()) {
            return Ok(());
        }
        let instance = ContainerOccurrence {
            system: &self.name_cell,
            root_entry: self.root_entry,
            root_inputs: &self.root,
            route: path.route.as_deref(),
            parent_route: path.parent_instance.as_deref(),
            declared: path.via.as_ref(),
            local: path.local,
            world: path.transform,
        };
        let row = instance.project(file, self.capture)?;
        self.read
            .location_physics
            .record(file.reference(), &file.location_physics)?;
        match &mut self.rows {
            PlacementRows::Spool { containers, .. } => {
                containers.append(&row, self.cancel).map_err(Into::into)
            }
            #[cfg(test)]
            PlacementRows::Typed => {
                self.read.instances.push(row);
                Ok(())
            }
            PlacementRows::SelectedContainers(rows) => {
                rows.instances.push(row);
                Ok(())
            }
            PlacementRows::Harvest(_) => Ok(()),
        }
    }

    pub(super) fn record_location(
        &mut self,
        child: &Child,
        path: &mut WalkPath,
        pose: AuthoredPose,
        file: &ContainerFile,
    ) -> Result<(), CatalogError> {
        let Some(location) = &child.location else {
            return Ok(());
        };
        let route = path
            .route
            .as_ref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let occurrence = LocationOccurrence {
            root_entry: self.root_entry,
            root_inputs: &self.root,
            route,
            parent_location: path.parent_location.as_deref(),
            container_route: path.parent_instance.as_deref(),
            local: pose,
            world: path.transform,
        };
        match &mut self.rows {
            PlacementRows::Spool { locations, .. } => {
                locations.append(
                    &occurrence.project(location, file, self.capture)?,
                    self.cancel,
                )?;
            }
            #[cfg(test)]
            PlacementRows::Typed => {
                self.read
                    .locations
                    .push(occurrence.project(location, file, self.capture)?);
            }
            PlacementRows::Harvest(_) | PlacementRows::SelectedContainers(_) => return Ok(()),
        }
        path.parent_location = Some(Arc::clone(route));
        Ok(())
    }
}
