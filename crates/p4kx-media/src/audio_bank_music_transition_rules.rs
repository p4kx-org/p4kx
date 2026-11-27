// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankHircMusicPlaylist, BankHircMusicSwitch, BankHircObject, BankMusicPlaylistRead,
    BankMusicSwitchRead, BankMusicTransitionFields, BankMusicTransitionRows,
};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_music_transition_rule_payload::{
    MusicTransitionRulePayload, SUMMARY, TransitionRuleSummary,
};
use crate::audio_bank_music_transition_rule_rows;
use crate::audio_bank_node_declarations::{Coverage, Declarations, Disposition};
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaWriter, Subresource};

pub(crate) enum Owner<'owner, 'source, R> {
    Switch(&'owner mut BankHircMusicSwitch<'source, R>),
    Playlist(&'owner mut BankHircMusicPlaylist<'source, R>),
}

struct Observed<'fields> {
    object: BankHircObject,
    fields: &'fields BankMusicTransitionFields,
    disposition: Disposition<'fields>,
}

impl<R> Owner<'_, '_, R> {
    fn observed(&self) -> Result<Observed<'_>, MediaError> {
        match self {
            Self::Switch(switch) => {
                let disposition = match &switch.status {
                    BankMusicSwitchRead::Decoded => Disposition::Observed(None),
                    BankMusicSwitchRead::Malformed(error) => Disposition::Observed(Some(error)),
                    BankMusicSwitchRead::Unsupported(_) => Disposition::Unsupported,
                    _ => {
                        return Err(MediaError::Contract(
                            "unexpected MusicSwitch transition disposition".into(),
                        ));
                    }
                };
                Ok(Observed {
                    object: switch.object,
                    fields: &switch.fields.transitions,
                    disposition,
                })
            }
            Self::Playlist(playlist) => {
                let disposition = match &playlist.status {
                    BankMusicPlaylistRead::Decoded => Disposition::Observed(None),
                    BankMusicPlaylistRead::Malformed(error) => Disposition::Observed(Some(error)),
                    BankMusicPlaylistRead::Unsupported(_) => Disposition::Unsupported,
                    _ => {
                        return Err(MediaError::Contract(
                            "unexpected MusicPlaylist transition disposition".into(),
                        ));
                    }
                };
                Ok(Observed {
                    object: playlist.object,
                    fields: &playlist.fields.transitions,
                    disposition,
                })
            }
        }
    }

    fn rows(&mut self) -> Option<BankMusicTransitionRows<'_, R>> {
        match self {
            Self::Switch(switch) => switch.transition_rules(),
            Self::Playlist(playlist) => playlist.transition_rules(),
        }
    }
}

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    mut owner: Owner<'_, '_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let original = owner.observed()?;
    let declarations = Declarations::new(projection, &original.object, version);
    let sites = declarations.sites(NodeKind::MusicTransitionRules);
    summary(
        &declarations,
        &sites,
        original.disposition,
        Some(original.fields),
        writer,
    )?;
    if let Some(rows) = owner.rows() {
        audio_bank_music_transition_rule_rows::project(projection, &sites, rows, writer)?;
    }
    Ok(())
}

pub(crate) fn unsupported(
    projection: &BankProjection<'_>,
    version: VersionSite,
    object: &BankHircObject,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let declarations = Declarations::new(projection, object, version);
    let sites = declarations.sites(NodeKind::MusicTransitionRules);
    summary(
        &declarations,
        &sites,
        Disposition::Unsupported,
        None,
        writer,
    )
}

fn summary(
    declarations: &Declarations<'_, '_>,
    sites: &NodeSites,
    disposition: Disposition<'_>,
    original: Option<&BankMusicTransitionFields>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = sites.record(declarations.bank.input, writer, None)?;
    let (observed, coverage) = observations(sites, &mut prepared, original)?;
    let (record, status, issue) = declarations.summary(
        sites,
        &mut prepared,
        disposition,
        &coverage,
        observed,
        "audio.bank.invalid-music-transition-rules",
    )?;
    prepared.finish().write(
        writer,
        &Subresource::new(SUMMARY, sites.indices().to_vec()),
        status,
        &MusicTransitionRulePayload::Summary(&record),
        issue.as_slice(),
    )
}

fn observations(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: Option<&BankMusicTransitionFields>,
) -> Result<(TransitionRuleSummary, Coverage), MediaError> {
    let Some((count, range, complete_rules)) = original.and_then(|fields| {
        fields
            .rule_count
            .as_ref()
            .zip(fields.checked_range.as_ref())
            .map(|(count, range)| (count, range, fields.complete_rules))
    }) else {
        return Ok((TransitionRuleSummary::default(), Coverage::Unavailable));
    };
    let extent = count.range.start..range.end;
    sites.field(
        prepared,
        "observed/declared_count",
        &count.range,
        "musicTransitionRuleCount",
    )?;
    sites.range(prepared, "observed/checked_range", range)?;
    sites.converted(
        prepared,
        "observed/complete_rules",
        &extent,
        "musicTransitionCompleteRules",
    )?;
    let coverage = if complete_rules == count.value {
        Coverage::Complete(extent)
    } else {
        Coverage::Truncated(extent.start)
    };
    Ok((
        TransitionRuleSummary {
            declared_count: Some(count.into()),
            complete_rules: Some(complete_rules),
            checked_range: Some(range.into()),
        },
        coverage,
    ))
}
