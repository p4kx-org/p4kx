// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::{
    BankActorMixerFields, BankContainerChild, BankContainerRead, BankHircActorMixer, BankHircField,
    BankHircObject, BankRtpcRead,
};

use crate::audio_bank_actor_mixer_payload::{
    ActorMixerChildRecord, ActorMixerPayload, ActorMixerRecord, Layout,
};
use crate::audio_bank_inline_state::{self, InlineStateRead};
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_declarations::{self, Owner};
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_node_properties;
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_rtpc;
use crate::audio_bank_rtpc_payload::RtpcOwner;
use crate::audio_bank_state::StateProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(crate) const NAMESPACE: &str = "audio-bank-hirc-actor-mixer";
pub(crate) const CHILD_NAMESPACE: &str = "audio-bank-hirc-actor-mixer-child";

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    mut actor: BankHircActorMixer<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let object = *actor.object();
    let sites = NodeSites::new(&object, version, NodeKind::ActorMixer);
    let disposition = Disposition::read(actor.status())?;
    header(
        projection,
        &sites,
        &actor,
        version.version,
        &disposition,
        writer,
    )?;
    audio_bank_node_properties::actor_mixer(projection, version, &mut actor, writer)?;
    audio_bank_node_declarations::emit(projection, version, Owner::ActorMixer(&mut actor), writer)?;
    if matches!(disposition.layout, Layout::Unsupported) {
        return Ok(());
    }
    if let Some(children) = actor.children() {
        for child in children {
            let child = child.map_err(|cause| projection.replay_failure(cause))?;
            child_record(projection, &sites, &child, writer)?;
        }
    }
    project_node(projection, version, &object, &mut actor, writer)
}

fn header<R>(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    actor: &BankHircActorMixer<'_, R>,
    version: u32,
    disposition: &Disposition,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = sites.record(projection.input, writer, None)?;
    let object = actor.object();
    let fields = actor.fields();
    let body = object.body_offset()..object.body_end_exclusive();
    let evidence = (!matches!(disposition.layout, Layout::Unsupported)).then_some(&body);
    sites.summary(&mut prepared, evidence, disposition.issue.is_some())?;
    word(
        sites,
        &mut prepared,
        "effect_count",
        fields.node.effect_count.as_ref(),
        "actorMixerEffectCount",
    )?;
    word(
        sites,
        &mut prepared,
        "metadata_count",
        fields.node.metadata_count.as_ref(),
        "actorMixerMetadataCount",
    )?;
    word(
        sites,
        &mut prepared,
        "pre_parent_word",
        fields.node.pre_parent_word.as_ref(),
        "actorMixerPreParentWord",
    )?;
    word(
        sites,
        &mut prepared,
        "parent_id",
        fields.node.parent_id.as_ref(),
        "actorMixerParentId",
    )?;
    word(
        sites,
        &mut prepared,
        "child_count",
        fields.child_count.as_ref(),
        "actorMixerChildCount",
    )?;
    ranges(sites, &mut prepared, fields)?;
    let record = ActorMixerRecord::read(disposition.layout, version, fields);
    prepared.finish().write(
        writer,
        &Subresource::new(NAMESPACE, sites.indices().to_vec()),
        disposition.status,
        &ActorMixerPayload::BankHircActorMixer(&record),
        disposition.issue.as_slice(),
    )
}

fn ranges(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    fields: &BankActorMixerFields,
) -> Result<(), MediaError> {
    for (name, range) in [
        ("checked_node_range", fields.node_range.as_ref()),
        ("checked_children_range", fields.children.as_ref()),
        ("trailing_range", fields.trailing.as_ref()),
    ] {
        if let Some(range) = range {
            sites.range(prepared, name, range)?;
        }
    }
    if let (Some(count), Some(children)) = (&fields.child_count, &fields.children) {
        sites.converted(
            prepared,
            "checked_children",
            &(count.range.start..children.end),
            "actorMixerCheckedChildren",
        )?;
    }
    Ok(())
}

fn word<T>(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    name: &str,
    field: Option<&BankHircField<T>>,
    original: &'static str,
) -> Result<(), MediaError> {
    if let Some(field) = field {
        sites.field(prepared, name, &field.range, original)?;
    }
    Ok(())
}

fn child_record(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    child: &BankContainerChild,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let range = &child.child_id.range;
    let mut prepared = sites.record(projection.input, writer, Some(range))?;
    sites.field(&mut prepared, "child_id", range, "actorMixerChildId")?;
    sites.converted(&mut prepared, "ordinal", range, "actorMixerChildPosition")?;
    let ordinal = u64::from(child.ordinal);
    let record = ActorMixerChildRecord {
        ordinal: DecimalU64::new(ordinal),
        child_id: (&child.child_id).into(),
    };
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(CHILD_NAMESPACE, vec![section, object, ordinal]),
        Status::Complete,
        &ActorMixerPayload::BankHircActorMixerChild(&record),
        &[],
    )
}

fn project_node<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    object: &BankHircObject,
    actor: &mut BankHircActorMixer<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let state = StateProjection::new(projection.input, object, version);
    let read = actor
        .states()
        .map_or(InlineStateRead::Unavailable, InlineStateRead::Readable);
    audio_bank_inline_state::project(projection, &state, read, writer)?;
    let error = match actor.status() {
        BankContainerRead::Malformed(error) => Some(error.clone()),
        _ => None,
    };
    let read = match (actor.rtpc(), error) {
        (Some(rows), _) if rows.checked_curve_count() == rows.declared_curve_count().value => {
            BankRtpcRead::Readable(rows)
        }
        (Some(rows), Some(error)) => BankRtpcRead::Malformed { rows, error },
        (None, Some(error)) => BankRtpcRead::Unavailable(error),
        _ => {
            return Err(MediaError::Contract(
                "inconsistent ActorMixer RTPC observation".into(),
            ));
        }
    };
    audio_bank_rtpc::project(
        projection,
        version,
        object,
        RtpcOwner::ActorMixer,
        read,
        writer,
    )
}

struct Disposition {
    layout: Layout,
    status: Status,
    issue: Option<MediaIssue>,
}

impl Disposition {
    fn read(status: &BankContainerRead) -> Result<Self, MediaError> {
        let (layout, status, issue) = match status {
            BankContainerRead::Decoded => (Layout::Decoded, Status::Complete, None),
            BankContainerRead::Malformed(cause) => (
                Layout::Malformed,
                Status::Partial,
                Some(MediaIssue::new(
                    "audio.bank.invalid-actor-mixer",
                    "HIRC",
                    Severity::Error,
                    cause.to_string(),
                )),
            ),
            BankContainerRead::Unsupported(cause) => (
                Layout::Unsupported,
                Status::Complete,
                Some(MediaIssue::new(
                    "audio.bank.unsupported-actor-mixer",
                    "HIRC",
                    Severity::Info,
                    cause.to_string(),
                )),
            ),
            _ => {
                return Err(MediaError::Contract(
                    "unexpected ActorMixer disposition".into(),
                ));
            }
        };
        Ok(Self {
            layout,
            status,
            issue,
        })
    }
}
