// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use super::{System, WalkPath};
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
        self.read
            .instances
            .push(instance.project(file, self.capture)?);
        Ok(())
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
        self.read
            .locations
            .push(occurrence.project(location, file, self.capture)?);
        path.parent_location = Some(Arc::clone(route));
        Ok(())
    }
}
