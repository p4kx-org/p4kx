// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken};

use super::super::resource_topology_declarations;
use super::classes::ClassTargets;
use super::hosts::{RoomAttachment, RoomOwners};
use super::{
    ClassLookup, Definitions, IndexedDefinitions, InstalledConfigurations, InteriorRoomCoverage,
    Projection, RoomLookup, ShipContext, interior,
};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::EntryCapture;
use crate::contract_loadout::Loadout;

pub(crate) fn collect_room_attachments(
    loadout: &Loadout,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
    rooms: &mut RoomLookup<'_>,
) -> Result<Vec<RoomAttachment>, CatalogError> {
    let mut projection = Projection {
        definitions: Definitions::Indexed(IndexedDefinitions::new()),
        origins,
        cancel,
        emit_instance: &mut |_| Ok(()),
    };
    let mut attachments = Vec::new();
    projection.loadout(
        loadout,
        rooms,
        &mut attachments,
        &mut InstalledConfigurations::default(),
    )?;
    Ok(attachments)
}

pub(crate) fn project_room_coverage<'lookup>(
    archive: &Archive,
    input: &OriginalCatalogInput<'_, '_>,
    root: crate::loadout::Wanted,
    declarations: &resource_topology_declarations::ResourceTopologyDeclarations<'_>,
    attachments: &[RoomAttachment],
    cancel: Option<&CancelToken>,
    classes: &'lookup mut ClassLookup<'lookup>,
) -> Result<Vec<InteriorRoomCoverage>, CatalogError> {
    let rooms = RoomOwners::new(declarations)?;
    if rooms.is_empty() {
        return Ok(Vec::new());
    }
    let origins = input.origins();
    let mut project = Projection {
        definitions: Definitions::Indexed(IndexedDefinitions::new()),
        origins,
        cancel,
        emit_instance: &mut |_| Ok(()),
    };
    let root = ShipContext::read(origins, root.guid, root.record_index)?;
    let capture = EntryCapture::for_input(input);
    interior::coverage(
        &mut project,
        archive,
        &root,
        &rooms,
        attachments,
        &capture,
        ClassTargets::scoped(input.forge(), cancel, classes),
    )
}
