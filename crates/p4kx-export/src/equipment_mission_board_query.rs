// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::EquipmentSnapshot;
use super::component_query::equipment_component_query;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_v3::catalog_mission_board::MissionBoardComponents;

pub use super::ComponentOwner;
pub use crate::catalog_v3::catalog_mission_board::{BoardDetails, BoardType, MissionBoard};

equipment_component_query!(
    MissionBoardComponents,
    MissionBoard,
    MissionBoardSnapshot,
    with_mission_boards,
    with_mission_boards_from
);
