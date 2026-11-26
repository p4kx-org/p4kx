// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter;

use p4kx_extraction::{Archive, CancelToken, PathKeys};

use super::super::resource_topology_declarations;
use super::classes::ClassTargets;
use super::hosts::{RoomAttachment, RoomHost, RoomOwners};
use super::{
    ClassLookup, Definitions, FacilityInstance, IndexedDefinitions, InstalledConfigurations,
    InteriorRoomCoverage, Projection, RoomLookup, ShipContext, interior,
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

pub(crate) struct RoomScope<'scope, 'source> {
    pub(crate) root: crate::loadout::Wanted,
    pub(crate) declarations:
        &'scope resource_topology_declarations::ResourceTopologyDeclarations<'source>,
    pub(crate) attachments: &'scope [RoomAttachment],
}

#[derive(Default)]
pub(crate) struct RoomProjection {
    pub(crate) coverage: Vec<InteriorRoomCoverage>,
    pub(crate) placements: Vec<FacilityInstance>,
}

pub(crate) fn project_rooms(
    archive: &Archive,
    input: &OriginalCatalogInput<'_, '_>,
    scope: RoomScope<'_, '_>,
    cancel: Option<&CancelToken>,
    classes: &mut dyn ClassLookup,
    include_placements: bool,
    path_keys: &mut Option<PathKeys>,
) -> Result<RoomProjection, CatalogError> {
    let rooms = RoomOwners::new(scope.declarations)?;
    if rooms.is_empty() {
        return Ok(RoomProjection::default());
    }
    let keys = path_keys.get_or_insert_with(|| PathKeys::new(archive));
    let origins = input.origins();
    let mut placements = Vec::new();
    let mut project = Projection {
        definitions: Definitions::Indexed(IndexedDefinitions::new()),
        origins,
        cancel,
        emit_instance: &mut |row| {
            placements.push(row);
            Ok(())
        },
    };
    let ship = ShipContext::read(origins, scope.root.guid, scope.root.record_index)?;
    let direct = RoomHost {
        record: scope.root.record_index,
        ship: &ship,
        attachment: None,
    };
    let hosts = iter::once(direct).chain(scope.attachments.iter().map(|attachment| RoomHost {
        record: attachment.record,
        ship: &attachment.ship,
        attachment: Some(attachment),
    }));
    let capture = EntryCapture::for_input(input);
    let forge = input.forge();
    let coverage = interior::scoped(
        &mut project,
        interior::RoomPaths { archive, keys },
        &rooms,
        hosts,
        &capture,
        ClassTargets::scoped(forge, cancel, classes),
        include_placements.then_some(forge),
    )?;
    Ok(RoomProjection {
        coverage,
        placements,
    })
}
