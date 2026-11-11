// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::DecimalU64;
use p4kx_extraction::HircKind;
use p4kx_extraction::bank::{
    BankHircObject, BankHircSemantic, BankHircSound, BankHircSoundNode, BankSource,
    BankSourceSection,
};

use crate::audio_bank_actor_mixer;
use crate::audio_bank_container_node;
use crate::audio_bank_failure::BankPhase;
use crate::audio_bank_inventory::{BankInventory, VersionSite};
use crate::audio_bank_music_track_controls;
use crate::audio_bank_node_declarations::{self, Owner};
use crate::audio_bank_node_properties;
use crate::audio_bank_payload::{BankEntryRange, BankHircRecord, BankObjectRecord, BankPayload};
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_rtpc;
use crate::audio_bank_rtpc_payload::RtpcOwner;
use crate::audio_bank_state::{self, StateProjection};
use crate::{
    MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource,
    audio_bank_action_properties, audio_bank_hirc_container, audio_bank_hirc_event,
    audio_bank_hirc_origins, audio_bank_hirc_sound, audio_bank_inline_state,
};
use crate::{
    audio_bank_music_playlist, audio_bank_music_segment, audio_bank_music_switch,
    audio_bank_music_track,
};

pub(crate) const HIRC_NAMESPACE: &str = "audio-bank-hirc";
pub(crate) const OBJECT_NAMESPACE: &str = "audio-bank-object";

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    source: &mut BankSource<R>,
    section: &BankSourceSection,
    inventory: &BankInventory,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut hierarchy = match source.hirc(section) {
        Ok(hierarchy) => hierarchy,
        Err(cause) => {
            return projection.failure(BankPhase::Hierarchy, Some(section), cause, writer);
        }
    };
    let header = inventory.observed_header();
    let version = header.map(|header| VersionSite {
        version: header.header().version,
        body_offset: header.section().body_offset(),
    });
    loop {
        let next = match header {
            Some(header) => hierarchy.next_semantic_with_source_fields(header),
            None => hierarchy
                .next()
                .map(|result| result.map(BankHircSemantic::Raw)),
        };
        let Some(result) = next else { break };
        let semantic = match result {
            Ok(semantic) => semantic,
            Err(cause) => {
                return projection.failure(BankPhase::Hierarchy, Some(section), cause, writer);
            }
        };
        emit_object(projection, semantic.object(), version, writer)?;
        emit_semantic(projection, semantic, version, inventory, writer)?;
    }
    let trailing = hierarchy.trailing_range().ok_or_else(|| {
        MediaError::Contract("completed HIRC cursor has no trailing range".into())
    })?;
    emit_summary(
        projection,
        section,
        hierarchy.object_count(),
        trailing,
        writer,
    )
}

fn emit_semantic<R: Read + Seek>(
    projection: &BankProjection<'_>,
    semantic: BankHircSemantic<'_, R>,
    version: Option<VersionSite>,
    inventory: &BankInventory,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    match (semantic, version) {
        (BankHircSemantic::Raw(object), version) => emit_raw(projection, &object, version, writer),
        (BankHircSemantic::ActorMixer(actor), Some(version)) => {
            audio_bank_actor_mixer::project(projection, version, *actor, writer)
        }
        (BankHircSemantic::Action(action), Some(version)) => {
            audio_bank_action_properties::unobserved(
                projection,
                version,
                &action,
                &inventory.hirc_objects,
                writer,
            )
        }
        (BankHircSemantic::ActionWithProperties(mut action), Some(version)) => {
            audio_bank_action_properties::play(
                projection,
                version,
                &mut action,
                &inventory.hirc_objects,
                writer,
            )
        }
        (BankHircSemantic::ContainerWithNode(container), Some(version)) => {
            audio_bank_hirc_container::project(projection, version, *container, writer)
        }
        (BankHircSemantic::MusicPlaylist(playlist), Some(version)) => {
            audio_bank_music_playlist::project(projection, version, *playlist, writer)
        }
        (BankHircSemantic::MusicSegment(segment), Some(version)) => {
            audio_bank_music_segment::project(projection, version, *segment, writer)
        }
        (BankHircSemantic::MusicSwitch(switch), Some(version)) => {
            audio_bank_music_switch::project(projection, version, *switch, writer)
        }
        (BankHircSemantic::MusicTrack(track), Some(version)) => {
            audio_bank_music_track::project(projection, version, *track, writer)
        }
        (BankHircSemantic::Event(event), Some(version)) => audio_bank_hirc_event::project(
            projection,
            version,
            event,
            &inventory.hirc_objects,
            writer,
        ),
        (BankHircSemantic::Sound(sound), Some(version)) => {
            emit_unscoped_sound(projection, version, &sound, inventory, writer)
        }
        (BankHircSemantic::SoundWithNode(node), Some(version)) => {
            emit_sound_node(projection, version, node, inventory, writer)
        }
        (BankHircSemantic::State(state), Some(version)) => {
            audio_bank_state::project(projection.input, version, state, writer)
        }
        _ => Err(MediaError::Contract(
            "HIRC semantics have no observed bank version".into(),
        )),
    }
}

fn emit_raw(
    projection: &BankProjection<'_>,
    original: &BankHircObject,
    version: Option<VersionSite>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let Some(version) = version else {
        return Ok(());
    };
    match original.kind(version.version) {
        HircKind::Sound => {
            audio_bank_node_properties::unsupported(projection, version, original, writer)?;
            audio_bank_node_declarations::unsupported(projection, version, original, writer)
        }
        HircKind::RandomSequenceContainer
        | HircKind::SwitchContainer
        | HircKind::LayerContainer => {
            audio_bank_container_node::unsupported(projection, version, original, writer)
        }
        HircKind::MusicTrack => {
            audio_bank_music_track_controls::unsupported(projection, version, original, writer)
        }
        _ => Ok(()),
    }
}

fn emit_unscoped_sound(
    projection: &BankProjection<'_>,
    version: VersionSite,
    sound: &BankHircSound,
    inventory: &BankInventory,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    audio_bank_hirc_sound::project(projection, version, sound, &inventory.didx_entries, writer)?;
    audio_bank_node_properties::unsupported(projection, version, &sound.object, writer)?;
    audio_bank_node_declarations::unsupported(projection, version, &sound.object, writer)
}

fn emit_sound_node<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    mut node: BankHircSoundNode<'_, R>,
    inventory: &BankInventory,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sound = node.sound();
    audio_bank_hirc_sound::project(projection, version, sound, &inventory.didx_entries, writer)?;
    let original = sound.object;
    audio_bank_node_properties::sound(projection, version, &mut node, writer)?;
    audio_bank_node_declarations::emit(projection, version, Owner::Sound(&mut node), writer)?;
    let state = StateProjection::new(projection.input, &original, version);
    audio_bank_inline_state::project(projection, &state, node.states().try_into()?, writer)?;
    audio_bank_rtpc::project(
        projection,
        version,
        &original,
        RtpcOwner::Sound,
        node.rtpc(),
        writer,
    )
}

fn emit_object(
    projection: &BankProjection<'_>,
    object: &BankHircObject,
    version: Option<VersionSite>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let payload = BankPayload::BankObject(BankObjectRecord {
        section_ordinal: DecimalU64::new(object.section_ordinal()),
        ordinal: DecimalU64::new(object.ordinal()),
        type_code: object.type_code(),
        kind: version.map(|version| object.kind(version.version).slug()),
        object_id: object.id(),
        header_range: BankEntryRange::new(object.header_offset(), object.body_offset()),
        body_range: BankEntryRange::new(object.body_offset(), object.body_end_exclusive()),
    });
    audio_bank_hirc_origins::object(projection.input, object, version, writer)?.write(
        writer,
        &Subresource::new(
            OBJECT_NAMESPACE,
            vec![object.section_ordinal(), object.ordinal()],
        ),
        Status::Complete,
        &payload,
        &[],
    )
}

fn emit_summary(
    projection: &BankProjection<'_>,
    section: &BankSourceSection,
    object_count: u32,
    trailing: Range<u64>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let has_trailing = !trailing.is_empty();
    let payload = BankPayload::BankHirc(BankHircRecord {
        section_ordinal: DecimalU64::new(section.ordinal()),
        declared_objects: object_count,
        trailing_range: BankEntryRange::new(trailing.start, trailing.end),
    });
    let issue = has_trailing.then(|| {
        MediaIssue::new(
            "audio.bank.trailing-hirc-bytes",
            "HIRC",
            Severity::Info,
            format!(
                "{} original bytes follow the declared objects",
                trailing.end - trailing.start
            ),
        )
    });
    audio_bank_hirc_origins::summary(projection.input, section, has_trailing, writer)?.write(
        writer,
        &Subresource::new(HIRC_NAMESPACE, vec![section.ordinal()]),
        Status::Complete,
        &payload,
        issue.as_slice(),
    )
}
