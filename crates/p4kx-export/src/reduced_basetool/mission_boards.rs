// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::component::{self, Component, ComponentKey};
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use crate::catalog_queries::equipment::mission_boards::{BoardDetails, BoardType, MissionBoard};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::source_value_catalog::SourceRecordReference;

pub(super) const PATH: &str = "configurations.missionBoards";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    faction_reputations: &RecordTargets,
) -> Result<StagedCatalogTable, ReducedError> {
    parsed.with_mission_boards(CatalogRecordSelection::All, cancel, |snapshot| {
        component::stage_selected(PATH, &snapshot, scratch, cancel, |row| {
            Board::read(row, faction_reputations)
        })
    })?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Board<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    board_type: Cell<Selection<'read>>,
}

impl<'read> Board<'read> {
    fn read(
        original: &'read MissionBoard<'_>,
        factions: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        ComponentKey::read(original.owner(), PATH)?;
        Ok(Self {
            owner: original.owner().into(),
            board_type: completeness::cell(PATH, original.board_type())?
                .try_map(|board| Selection::read(board, factions))?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Selection<'read> {
    source_type: Cell<&'read str>,
    details: Details<'read>,
}

impl<'read> Selection<'read> {
    fn read(source: &'read BoardType<'_>, factions: &RecordTargets) -> Result<Self, ReducedError> {
        Ok(Self {
            source_type: completeness::cell(PATH, source.source_type())?.map(|source| *source),
            details: match source.details() {
                BoardDetails::Base => Details::Base,
                BoardDetails::Notice => Details::Notice,
                BoardDetails::Authored => Details::Authored,
                BoardDetails::Career {
                    guild_faction_reputation,
                    factions: affiliations,
                } => Details::Career {
                    guild_faction_reputation: completeness::cell(PATH, guild_faction_reputation)?
                        .try_map(|link| faction(link, factions))?,
                    factions: completeness::list(PATH, affiliations, |link| {
                        faction(link, factions)
                    })?,
                },
            },
        })
    }
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Details<'read> {
    Base,
    Career {
        guild_faction_reputation: Cell<RecordLink<'read>>,
        factions: Cell<Vec<Cell<RecordLink<'read>>>>,
    },
    Notice,
    Authored,
}

fn faction<'read>(
    link: &'read SourceRecordReference<'_>,
    factions: &RecordTargets,
) -> Result<RecordLink<'read>, ReducedError> {
    RecordLink::read_source(PATH, link, factions, |_| true)
}
