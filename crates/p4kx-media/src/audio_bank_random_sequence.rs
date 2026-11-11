// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankHircContainerNode, BankHircField, BankHircObject, BankRandomSequenceFields,
};

use crate::audio_bank_container_node;
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_declarations::{Coverage, Declarations, Disposition};
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_random_sequence_payload::{
    RandomSequenceObservations, RandomSequencePayload, SUMMARY_NAMESPACE, TransitionWord,
};
use crate::audio_bank_random_sequence_rows;
use crate::{MediaError, MediaWriter, Subresource};

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    container: &mut BankHircContainerNode<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let declarations = Declarations::new(projection, container.object(), version);
    let disposition = audio_bank_container_node::disposition(container.status())?;
    let fields = container.fields();
    summary(
        &declarations,
        disposition,
        fields.random_sequence.as_ref(),
        fields.mode.as_ref(),
        writer,
    )?;
    if let Some(rows) = container.playlist() {
        audio_bank_random_sequence_rows::project(&declarations, rows, writer)?;
    }
    Ok(())
}

pub(crate) fn unsupported(
    projection: &BankProjection<'_>,
    version: VersionSite,
    original: &BankHircObject,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let declarations = Declarations::new(projection, original, version);
    summary(&declarations, Disposition::Unsupported, None, None, writer)
}

fn summary(
    declarations: &Declarations<'_, '_>,
    disposition: Disposition<'_>,
    original: Option<&BankRandomSequenceFields>,
    mode: Option<&BankHircField<u8>>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = declarations.sites(NodeKind::RandomSequence);
    let mut prepared = sites.record(declarations.bank.input, writer, None)?;
    let observed = if let Some(fields) = original {
        observations(&sites, &mut prepared, fields, mode)?
    } else {
        RandomSequenceObservations::default()
    };
    let coverage = Coverage::read(
        original.map(|fields| fields.start),
        original.and_then(BankRandomSequenceFields::complete_range),
    );
    let (payload, status, issue) = declarations.summary(
        &sites,
        &mut prepared,
        disposition,
        &coverage,
        observed,
        "audio.bank.invalid-random-sequence",
    )?;
    prepared.finish().write(
        writer,
        &Subresource::new(SUMMARY_NAMESPACE, sites.indices().to_vec()),
        status,
        &RandomSequencePayload::Summary(&payload),
        issue.as_slice(),
    )
}

fn observations(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    fields: &BankRandomSequenceFields,
    mode: Option<&BankHircField<u8>>,
) -> Result<RandomSequenceObservations, MediaError> {
    let mut observed = controls(sites, prepared, fields, mode)?;
    observed.transition_time = transition(
        sites,
        prepared,
        "transition_time",
        fields.transition_time.as_ref(),
        "randomSequenceTransitionTime",
    )?;
    observed.transition_minimum = transition(
        sites,
        prepared,
        "transition_minimum",
        fields.transition_minimum.as_ref(),
        "randomSequenceTransitionMinimum",
    )?;
    observed.transition_maximum = transition(
        sites,
        prepared,
        "transition_maximum",
        fields.transition_maximum.as_ref(),
        "randomSequenceTransitionMaximum",
    )?;
    if let Some(playlist) = &fields.playlist {
        sites.field(
            prepared,
            "observed/playlist_count",
            &playlist.declared_count.range,
            "randomSequencePlaylistCount",
        )?;
        sites.range(prepared, "observed/playlist_range", &playlist.checked_range)?;
        observed.playlist_count = Some((&playlist.declared_count).into());
        observed.playlist_range = Some((&playlist.checked_range).into());
    }
    Ok(observed)
}

fn controls(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    fields: &BankRandomSequenceFields,
    mode: Option<&BankHircField<u8>>,
) -> Result<RandomSequenceObservations, MediaError> {
    Ok(RandomSequenceObservations {
        loop_count: sites.scalar(
            prepared,
            "observed/loop_count",
            fields.loop_count.as_ref(),
            "randomSequenceLoopCount",
        )?,
        loop_minimum: sites.scalar(
            prepared,
            "observed/loop_minimum",
            fields.loop_minimum.as_ref(),
            "randomSequenceLoopMinimum",
        )?,
        loop_maximum: sites.scalar(
            prepared,
            "observed/loop_maximum",
            fields.loop_maximum.as_ref(),
            "randomSequenceLoopMaximum",
        )?,
        avoid_repeat_count: sites.scalar(
            prepared,
            "observed/avoid_repeat_count",
            fields.avoid_repeat_count.as_ref(),
            "randomSequenceAvoidRepeatCount",
        )?,
        transition_mode: sites.scalar(
            prepared,
            "observed/transition_mode",
            fields.transition_mode.as_ref(),
            "randomSequenceTransitionMode",
        )?,
        random_mode: sites.scalar(
            prepared,
            "observed/random_mode",
            fields.random_mode.as_ref(),
            "randomSequenceRandomMode",
        )?,
        mode: sites.scalar(prepared, "observed/mode", mode, "randomSequenceMode")?,
        flags: sites.scalar(
            prepared,
            "observed/flags",
            fields.flags.as_ref(),
            "randomSequenceFlags",
        )?,
        ..RandomSequenceObservations::default()
    })
}

fn transition(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    name: &str,
    field: Option<&BankHircField<u32>>,
    label: &'static str,
) -> Result<Option<TransitionWord>, MediaError> {
    let Some(field) = field else { return Ok(None) };
    let path = format!("observed/{name}");
    sites.field(prepared, &format!("{path}/bits"), &field.range, label)?;
    let value = f32::from_bits(field.value);
    let finite_value = value.is_finite().then_some(value);
    if finite_value.is_some() {
        sites.converted(
            prepared,
            &format!("{path}/finite_value"),
            &field.range,
            "randomSequenceFiniteTransition",
        )?;
    }
    Ok(Some(TransitionWord {
        bits: field.into(),
        finite_value,
    }))
}
