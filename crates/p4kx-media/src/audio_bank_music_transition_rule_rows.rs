// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::bank::{
    BankMusicTransitionId, BankMusicTransitionRows, BankMusicTransitionRule,
    BankMusicTransitionSide,
};

use crate::audio_bank_music_transition_rule_fields;
use crate::audio_bank_music_transition_rule_payload::{
    ID, MusicTransitionRulePayload, RULE, TransitionIdRecord, TransitionIdSide,
    TransitionRuleLayout,
};
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

struct RuleSite {
    ordinal: u64,
    count: Range<u64>,
}

impl TransitionIdSide {
    const fn original(self) -> BankMusicTransitionSide {
        match self {
            Self::Source => BankMusicTransitionSide::Source,
            Self::Destination => BankMusicTransitionSide::Destination,
        }
    }

    const fn index(self) -> u64 {
        match self {
            Self::Source => 0,
            Self::Destination => 1,
        }
    }
}

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    mut rows: BankMusicTransitionRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    while let Some(mut row) = rows
        .next_rule()
        .map_err(|cause| projection.replay_failure(cause))?
    {
        let site = RuleSite {
            ordinal: u64::from(row.ordinal),
            count: row.fields.source_ids.declared_count.range.clone(),
        };
        rule(projection, sites, &site, &row, writer)?;
        ids(projection, sites, &site, &mut row, writer)?;
    }
    Ok(())
}

fn rule<R>(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    site: &RuleSite,
    row: &BankMusicTransitionRule<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = sites.record(projection.input, writer, Some(&site.count))?;
    sites.index(&mut prepared, 2, &site.count, "musicTransitionRulePosition")?;
    sites.converted(
        &mut prepared,
        "layout",
        &row.checked_range,
        "musicTransitionRuleCoverage",
    )?;
    let (layout, status, issue) = if let Some(error) = &row.issue {
        sites.issues(&mut prepared, &row.checked_range)?;
        (
            TransitionRuleLayout::Malformed,
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.truncated-music-transition-rule",
                "HIRC",
                Severity::Error,
                error.to_string(),
            )),
        )
    } else {
        (TransitionRuleLayout::Decoded, Status::Complete, None)
    };
    let record =
        audio_bank_music_transition_rule_fields::read(sites, &mut prepared, &row.fields, layout)?;
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(RULE, vec![section, object, site.ordinal]),
        status,
        &MusicTransitionRulePayload::Rule(&record),
        issue.as_slice(),
    )
}

struct ListSite {
    side: TransitionIdSide,
    count: Range<u64>,
}

fn ids<R: Read + Seek>(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    rule: &RuleSite,
    row: &mut BankMusicTransitionRule<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for side in [TransitionIdSide::Source, TransitionIdSide::Destination] {
        let list = match side {
            TransitionIdSide::Source => Some(&row.fields.source_ids),
            TransitionIdSide::Destination => row.fields.destination_ids.as_ref(),
        };
        let Some(list) = list else { continue };
        let list = ListSite {
            side,
            count: list.declared_count.range.clone(),
        };
        let Some(ids) = row.ids(side.original()) else {
            return Err(MediaError::Contract(
                "observed transition ID list has no replay cursor".into(),
            ));
        };
        for id in ids {
            let id = id.map_err(|cause| projection.replay_failure(cause))?;
            id_row(projection, sites, rule, &list, &id, writer)?;
        }
    }
    Ok(())
}

fn id_row(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    rule: &RuleSite,
    list: &ListSite,
    original: &BankMusicTransitionId,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let range = &original.id.range;
    let mut prepared = sites.record(projection.input, writer, Some(range))?;
    sites.index(&mut prepared, 2, &rule.count, "musicTransitionRulePosition")?;
    sites.index(
        &mut prepared,
        3,
        &list.count,
        "musicTransitionIdListPosition",
    )?;
    sites.index(&mut prepared, 4, range, "musicTransitionIdPosition")?;
    sites.converted(&mut prepared, "list", &list.count, "musicTransitionIdList")?;
    sites.field(&mut prepared, "id", range, "musicTransitionReferenceId")?;
    let [section, object] = sites.indices();
    let record = TransitionIdRecord {
        list: list.side,
        id: (&original.id).into(),
    };
    prepared.finish().write(
        writer,
        &Subresource::new(
            ID,
            vec![
                section,
                object,
                rule.ordinal,
                list.side.index(),
                u64::from(original.ordinal),
            ],
        ),
        Status::Complete,
        &MusicTransitionRulePayload::Id(&record),
        &[],
    )
}
