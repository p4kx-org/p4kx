// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::{
    BankMusicTransitionDestinationFields, BankMusicTransitionIds, BankMusicTransitionObjectFields,
    BankMusicTransitionRuleFields, BankMusicTransitionSourceFields,
};

use crate::MediaError;
use crate::audio_bank_music_transition_fade::{self, FadeSite};
use crate::audio_bank_music_transition_rule_payload::{
    TransitionDestination, TransitionIdCoverage, TransitionIdTable, TransitionObject,
    TransitionRuleLayout, TransitionRuleRecord, TransitionSource,
};
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;

pub(crate) fn read(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankMusicTransitionRuleFields,
    layout: TransitionRuleLayout,
) -> Result<TransitionRuleRecord, MediaError> {
    let source_ids = ids(
        sites,
        prepared,
        &original.source_ids,
        "source_ids",
        "musicTransitionSourceIdCount",
    )?;
    let destination_ids = original
        .destination_ids
        .as_ref()
        .map(|table| {
            ids(
                sites,
                prepared,
                table,
                "destination_ids",
                "musicTransitionDestinationIdCount",
            )
        })
        .transpose()?;
    let transition_object = original
        .transition_object
        .as_ref()
        .map(|fields| object(sites, prepared, fields))
        .transpose()?
        .flatten();
    Ok(TransitionRuleRecord {
        layout,
        source_ids,
        destination_ids,
        source: source(sites, prepared, &original.source)?,
        destination: destination(sites, prepared, &original.destination)?,
        allocation_flag: sites.scalar(
            prepared,
            "allocation_flag",
            original.allocation_flag.as_ref(),
            "musicTransitionAllocationFlag",
        )?,
        transition_object,
    })
}

fn ids(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankMusicTransitionIds,
    path: &str,
    label: &'static str,
) -> Result<TransitionIdTable, MediaError> {
    let count = &original.declared_count;
    let range = &original.checked_range;
    sites.field(
        prepared,
        &format!("{path}/declared_count"),
        &count.range,
        label,
    )?;
    sites.range(prepared, &format!("{path}/checked_range"), range)?;
    let coverage = if original.is_complete() {
        TransitionIdCoverage::Complete
    } else {
        TransitionIdCoverage::Truncated
    };
    sites.converted(
        prepared,
        &format!("{path}/coverage"),
        &(count.range.start..range.end),
        "musicTransitionIdCoverage",
    )?;
    Ok(TransitionIdTable {
        declared_count: count.into(),
        checked_range: range.into(),
        coverage,
    })
}

fn source(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankMusicTransitionSourceFields,
) -> Result<Option<TransitionSource>, MediaError> {
    let Some(fade) =
        audio_bank_music_transition_fade::read(sites, prepared, &original.fade, FadeSite::Source)?
    else {
        return Ok(None);
    };
    Ok(Some(TransitionSource {
        fade,
        sync_type: sites.scalar(
            prepared,
            "source/sync_type",
            original.sync_type.as_ref(),
            "musicTransitionSourceSyncType",
        )?,
        cue_filter_hash: sites.scalar(
            prepared,
            "source/cue_filter_hash",
            original.cue_filter_hash.as_ref(),
            "musicTransitionSourceCueFilterHash",
        )?,
        play_post_exit: sites.scalar(
            prepared,
            "source/play_post_exit",
            original.play_post_exit.as_ref(),
            "musicTransitionSourcePlayPostExit",
        )?,
    }))
}

fn destination(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankMusicTransitionDestinationFields,
) -> Result<Option<TransitionDestination>, MediaError> {
    let Some(fade) = audio_bank_music_transition_fade::read(
        sites,
        prepared,
        &original.fade,
        FadeSite::Destination,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(TransitionDestination {
        fade,
        cue_filter_hash: sites.scalar(
            prepared,
            "destination/cue_filter_hash",
            original.cue_filter_hash.as_ref(),
            "musicTransitionDestinationCueFilterHash",
        )?,
        jump_to_id: sites.scalar(
            prepared,
            "destination/jump_to_id",
            original.jump_to_id.as_ref(),
            "musicTransitionJumpToId",
        )?,
        jump_to_type: sites.scalar(
            prepared,
            "destination/jump_to_type",
            original.jump_to_type.as_ref(),
            "musicTransitionJumpToType",
        )?,
        entry_type: sites.scalar(
            prepared,
            "destination/entry_type",
            original.entry_type.as_ref(),
            "musicTransitionEntryType",
        )?,
        play_pre_entry: sites.scalar(
            prepared,
            "destination/play_pre_entry",
            original.play_pre_entry.as_ref(),
            "musicTransitionDestinationPlayPreEntry",
        )?,
        match_source_cue_name: sites.scalar(
            prepared,
            "destination/match_source_cue_name",
            original.match_source_cue_name.as_ref(),
            "musicTransitionMatchSourceCueName",
        )?,
    }))
}

fn object(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankMusicTransitionObjectFields,
) -> Result<Option<TransitionObject>, MediaError> {
    let Some(id) = &original.segment_id else {
        return Ok(None);
    };
    sites.field(
        prepared,
        "transition_object/segment_id",
        &id.range,
        "musicTransitionObjectSegmentId",
    )?;
    Ok(Some(TransitionObject {
        segment_id: id.into(),
        fade_in: audio_bank_music_transition_fade::read(
            sites,
            prepared,
            &original.fade_in,
            FadeSite::ObjectIn,
        )?,
        fade_out: audio_bank_music_transition_fade::read(
            sites,
            prepared,
            &original.fade_out,
            FadeSite::ObjectOut,
        )?,
        play_pre_entry: sites.scalar(
            prepared,
            "transition_object/play_pre_entry",
            original.play_pre_entry.as_ref(),
            "musicTransitionObjectPlayPreEntry",
        )?,
        play_post_exit: sites.scalar(
            prepared,
            "transition_object/play_post_exit",
            original.play_post_exit.as_ref(),
            "musicTransitionObjectPlayPostExit",
        )?,
    }))
}
