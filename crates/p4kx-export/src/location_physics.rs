// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Arc, OnceLock};

use p4kx_extraction::model::{
    SocLocationPhysics, SocPhysicsDeclaration, SocPhysicsKind, SocSourceValue,
};
use p4kx_extraction::{PathKeys, SourceUse};

use crate::catalog_read::authored_xml_tree;
use crate::catalog_read::original::{EntryRead, EntrySourceXml, EntryXml};
use crate::starmap_parse::entxml::EntXmlFiles;
use crate::starmap_xml::{self, Node};
use crate::{CatalogError, ExportAccountingError};

#[path = "location_physics_descriptor_query.rs"]
mod descriptor_query;
#[path = "location_physics_rows.rs"]
mod rows;
#[path = "location_physics_stage.rs"]
pub(crate) mod stage;

#[path = "location_physics_targets.rs"]
mod targets;

#[cfg(test)]
#[path = "location_physics_installed_witness.rs"]
mod installed_witness;

pub use descriptor_query::{
    LocationPhysicsDescriptorSelection, LocationPhysicsDescriptorSnapshot,
    LocationPhysicsDescriptorStatus, read_location_physics_descriptor,
    read_location_physics_descriptor_with_provenance_mode,
};
pub use rows::{
    LocationPhysicsDeclaration, LocationPhysicsFields, LocationPhysicsGrid,
    LocationPhysicsGridControls, LocationVoxelTarget,
};
pub(crate) use stage::GridObservations;

use targets::Directories;

#[derive(Debug, Default)]
pub(crate) enum GridRead {
    #[default]
    Unavailable,
    Absent,
    Present(Arc<LocationPhysicsGrid>),
}

pub(crate) fn read(
    read: &EntryRead,
    placement: &EntryXml<'_, '_, '_>,
    files: &EntXmlFiles<'_, '_>,
    root_keys: &OnceLock<PathKeys>,
    directory: Option<&SourceUse>,
) -> Result<GridRead, CatalogError> {
    let root = placement.root();
    if placement.is_lossy()
        || matches!(root, starmap_xml::Root::Text(node) if node.tag_name().namespace().is_some())
        || !root.name().eq_ignore_ascii_case("ObjectContainer")
    {
        return Ok(GridRead::Unavailable);
    }
    if starmap_xml::attribute(root, "gridRadius").is_none()
        && !root
            .children()
            .any(|node| matches!(node.name(), "PhysicsGrid" | "PhysicsGridPart"))
    {
        return Ok(GridRead::Absent);
    }
    let Ok(xml) = EntrySourceXml::parse(read, files.capture)? else {
        return Ok(GridRead::Unavailable);
    };
    let Some(physics) = SocLocationPhysics::from_root(xml.root()).filter(|_| !xml.is_lossy())
    else {
        return Ok(GridRead::Unavailable);
    };
    let radius = physics.grid_radius();
    if matches!(radius.into_value(), SocSourceValue::Absent)
        && physics.declarations().next().is_none()
    {
        return Ok(GridRead::Absent);
    }
    let keys = root_keys.get_or_init(|| PathKeys::new(files.archive));
    let inner = PathKeys::new(files.container);
    let paths = [inner.index(files.container), keys.index(files.archive)];
    let directories = Directories {
        paths: &paths,
        local_proof: targets::observe_container_directory(files.container, xml.sources())?,
        root_proof: directory,
        cancel: files.cancel,
    };
    Ok(GridRead::Present(Arc::new(project(
        &xml,
        physics,
        placement,
        files,
        &directories,
    )?)))
}

fn project(
    xml: &EntrySourceXml<'_>,
    physics: SocLocationPhysics<'_>,
    placement: &EntryXml<'_, '_, '_>,
    files: &EntXmlFiles<'_, '_>,
    directories: &Directories<'_, '_>,
) -> Result<LocationPhysicsGrid, CatalogError> {
    let origins = xml.sources();
    let container = placement
        .containing_entry()?
        .physical_reference(files.capture)?
        .into_cell(origins);
    let descriptor = placement.payload_reference()?.into_cell(origins);
    let source_elements = authored_xml_tree::visit_roots(
        xml,
        physics.declarations().map(SocPhysicsDeclaration::element),
        files.cancel,
        |_, _| Ok(()),
    )?;
    let declarations = all_declarations(physics, xml, directories)?;
    Ok(LocationPhysicsGrid {
        container,
        descriptor,
        grid_radius: xml.physics_field(physics.grid_radius())?,
        declarations,
        source_elements,
    })
}

fn all_declarations(
    physics: SocLocationPhysics<'_>,
    xml: &EntrySourceXml<'_>,
    directories: &Directories<'_, '_>,
) -> Result<Vec<LocationPhysicsDeclaration>, CatalogError> {
    let mut pending: Vec<_> = physics.declarations().collect();
    pending.reverse();
    let mut output = Vec::new();
    while let Some(node) = pending.pop() {
        crate::catalog_query_session::checkpoint(directories.cancel)?;
        let position = xml.position(node.element())?;
        let owner = xml.position(node.owner())?;
        let sibling =
            u64::try_from(node.ordinal()).map_err(|_| ExportAccountingError::CounterOverflow)?;
        output.push(LocationPhysicsDeclaration {
            sibling_ordinal: position.map_ref(|_| sibling),
            position,
            owner,
            fields: fields(node, xml, directories)?,
        });
        let start = pending.len();
        pending.extend(node.children());
        pending
            .get_mut(start..)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?
            .reverse();
    }
    Ok(output)
}

fn fields(
    node: SocPhysicsDeclaration<'_>,
    xml: &EntrySourceXml<'_>,
    directories: &Directories<'_, '_>,
) -> Result<LocationPhysicsFields, CatalogError> {
    match node.kind() {
        SocPhysicsKind::Grid => {
            let grid = node
                .grid()
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            Ok(LocationPhysicsFields::Grid {
                controls: Box::new(LocationPhysicsGridControls {
                    inherit_gravity: xml.physics_field(grid.inherit_gravity())?,
                    gravity: xml.physics_field(grid.gravity())?,
                    grid_type: xml.physics_field(grid.grid_type())?.map(str::to_owned),
                    cell_size: xml.physics_field(grid.cell_size())?,
                    has_part: xml.physics_field(grid.has_part())?,
                    portal_exclusive_mode: xml.physics_field(grid.portal_exclusive_mode())?,
                }),
            })
        }
        SocPhysicsKind::Part => {
            let shape = node
                .shape()
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            Ok(LocationPhysicsFields::Part {
                shape: xml.physics_field(shape)?.map(str::to_owned),
            })
        }
        SocPhysicsKind::Voxel => {
            let filename = node
                .filename()
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            Ok(LocationPhysicsFields::Voxel {
                filename: xml.physics_field(filename)?.map(str::to_owned),
                target: directories.voxel(xml, &filename)?,
            })
        }
        SocPhysicsKind::ChildParts => Ok(LocationPhysicsFields::ChildParts),
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
}
