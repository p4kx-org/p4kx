// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};

use p4kx_audio::{
    BankHircMusicPlaylist, BankHircMusicSwitch, BankMusicNodeFields, BankMusicTransitionFields,
    BankMusicTransitionRows,
};
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::source_evidence_error::SourceEvidenceError;

#[path = "hirc_music_transition_source_dto.rs"]
mod dto;
#[path = "hirc_music_transition_source_rows.rs"]
mod rows;

pub(super) use self::dto::TransitionRulesSchema;
use self::dto::{TransitionLayout, TransitionRules};
use super::super::origins::ObjectOrigins;
use super::stingers::Stingers;
use super::{DocumentSite, Layout, playlist_stinger, switch_stinger};

pub(super) enum Owner<'input, R> {
    Playlist(Box<BankHircMusicPlaylist<'input, R>>),
    Switch(Box<BankHircMusicSwitch<'input, R>>),
}

impl<R> Owner<'_, R> {
    pub(super) fn fields(&self) -> (&BankMusicNodeFields, &BankMusicTransitionFields) {
        match self {
            Self::Playlist(node) => (&node.fields.node, &node.fields.transitions),
            Self::Switch(node) => (&node.fields.node, &node.fields.transitions),
        }
    }

    pub(super) fn transition_rows(&mut self) -> Option<BankMusicTransitionRows<'_, R>> {
        match self {
            Self::Playlist(node) => node.transition_rules(),
            Self::Switch(node) => node.transition_rules(),
        }
    }
}

pub(super) struct SharedStingers<'view, 'input, R> {
    owner: &'view RefCell<Owner<'input, R>>,
    origins: &'view ObjectOrigins,
}

impl<'view, 'input, R> SharedStingers<'view, 'input, R> {
    pub(super) const fn new(
        owner: &'view RefCell<Owner<'input, R>>,
        origins: &'view ObjectOrigins,
    ) -> Self {
        Self { owner, origins }
    }
}

impl<R: Read + Seek> Serialize for SharedStingers<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut owner = self.owner.try_borrow_mut().map_err(S::Error::custom)?;
        match &mut *owner {
            Owner::Playlist(node) => {
                let node = node.as_mut();
                Stingers::new(
                    &mut node.rows,
                    playlist_stinger,
                    node.fields.node.stinger_tail.take(),
                    self.origins,
                )
                .serialize(serializer)
            }
            Owner::Switch(node) => {
                let node = node.as_mut();
                Stingers::new(
                    &mut node.rows,
                    switch_stinger,
                    node.fields.node.stinger_tail.take(),
                    self.origins,
                )
                .serialize(serializer)
            }
        }
    }
}

pub(super) fn observed<'view, 'input, R>(
    site: &DocumentSite<'view>,
    layout: &Layout,
    fields: &BankMusicTransitionFields,
    owner: &'view RefCell<Owner<'input, R>>,
) -> Result<TransitionRules<rows::Rows<'view, 'input, R>>, SourceEvidenceError> {
    let rows = rows::Rows::new(owner, site.origins);
    let Some((count, range)) = fields
        .rule_count
        .as_ref()
        .zip(fields.checked_range.as_ref())
    else {
        return unobserved(site, layout, rows);
    };
    let extent = count.range.start..range.end;
    let layout = if fields.complete_rules == count.value {
        TransitionLayout::Decoded
    } else {
        TransitionLayout::Malformed
    };
    let origins = site.origins;
    Ok(TransitionRules {
        layout: origins.derived(layout, &extent, "musicTransitionTableCoverage")?,
        declared_count: Some(origins.field(count, "musicTransitionRuleCount")?),
        complete_rules: Some(origins.derived(
            fields.complete_rules,
            &extent,
            "musicTransitionCompleteRules",
        )?),
        complete_range: fields
            .rules_range
            .as_ref()
            .map(|range| origins.range(range, "musicTransitionCompleteRulesExtent"))
            .transpose()?,
        checked_range: Some(origins.range(range, "musicTransitionCheckedRulesExtent")?),
        rows,
    })
}

fn unobserved<Rows>(
    site: &DocumentSite<'_>,
    layout: &Layout,
    rows: Rows,
) -> Result<TransitionRules<Rows>, SourceEvidenceError> {
    let layout = if matches!(layout, Layout::Unsupported) {
        TransitionLayout::Unsupported
    } else {
        TransitionLayout::Unavailable
    };
    Ok(TransitionRules {
        layout: site.origins.coverage(layout, site.header, site.original)?,
        declared_count: None,
        complete_rules: None,
        complete_range: None,
        checked_range: None,
        rows,
    })
}

pub(super) fn unsupported(
    site: &DocumentSite<'_>,
) -> Result<TransitionRulesSchema, SourceEvidenceError> {
    unobserved(site, &Layout::Unsupported, Vec::new())
}
