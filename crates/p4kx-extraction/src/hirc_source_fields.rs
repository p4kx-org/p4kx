// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_audio::{
    BankHircDialogueEvent, BankHircObject, BankHircSemantic, BankObservedHeader, HircKind,
};
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::bank::Bank;

#[path = "hirc_action_property_source.rs"]
mod action;
#[path = "hirc_attenuation_source.rs"]
mod attenuation;
#[path = "hirc_blend_source.rs"]
mod blend;
#[path = "hirc_container_source_layout.rs"]
mod container_layout;
#[path = "hirc_dialogue_dto.rs"]
mod dto;
#[path = "hirc_effect_source.rs"]
mod effect_source;
#[path = "hirc_music_track_source.rs"]
mod music_track;
#[path = "hirc_source_origins.rs"]
pub(super) mod origins;
#[path = "hirc_random_sequence_source.rs"]
mod random_sequence;
#[path = "hirc_dialogue_rows.rs"]
mod rows;
#[path = "hirc_source_schema.rs"]
mod schema;
#[path = "hirc_standalone_state_source.rs"]
mod standalone_state;
#[path = "hirc_dialogue_summary.rs"]
mod summary;
#[path = "hirc_switch_source.rs"]
mod switch;

use self::dto::{
    DialogueSourceDocument, DialogueSourceKind, DialogueSourceLayout, DialogueSourceRow,
    DialogueSummary,
};
use self::origins::ObjectOrigins;
use self::rows::DialogueRows;

pub(super) use self::schema::ObjectSourceSchema;

#[derive(Debug, Clone, Copy)]
enum SourceFamily {
    Action,
    State,
    Dialogue,
    Attenuation,
    RandomSequence,
    Switch,
    Blend,
    MusicTrack,
    Plugin(HircKind),
    Modulator(HircKind),
}

impl SourceFamily {
    const fn kind(self) -> HircKind {
        match self {
            Self::Action => HircKind::Action,
            Self::State => HircKind::State,
            Self::Dialogue => HircKind::DialogueEvent,
            Self::Attenuation => HircKind::Attenuation,
            Self::RandomSequence => HircKind::RandomSequenceContainer,
            Self::Switch => HircKind::SwitchContainer,
            Self::Blend => HircKind::LayerContainer,
            Self::MusicTrack => HircKind::MusicTrack,
            Self::Plugin(kind) | Self::Modulator(kind) => kind,
        }
    }

    const fn namespace(self) -> &'static str {
        match self {
            Self::Action => "bnkHircActionProperties",
            Self::State => "bnkHircStandaloneState",
            Self::Dialogue => "bnkDialogueEvent",
            Self::Attenuation => "bnkAttenuation",
            Self::RandomSequence => "bnkHircRandomSequence",
            Self::Switch => "bnkHircSwitchContainer",
            Self::Blend => "bnkHircBlendContainer",
            Self::MusicTrack => "bnkHircMusicTrack",
            Self::Plugin(_) => "bnkHircPlugin",
            Self::Modulator(_) => "bnkHircModulator",
        }
    }

    const fn extent_label(self) -> &'static str {
        match self {
            Self::Action => "actionObjectExtent",
            Self::State => "stateObjectExtent",
            Self::Dialogue => "dialogueExtent",
            Self::Attenuation => "attenuationExtent",
            Self::RandomSequence => "randomSequenceExtent",
            Self::Switch => "switchContainerExtent",
            Self::Blend => "blendContainerExtent",
            Self::MusicTrack => "musicTrackExtent",
            Self::Plugin(_) => "pluginObjectExtent",
            Self::Modulator(_) => "modulatorObjectExtent",
        }
    }
}

/// A physical HIRC declaration read from the owning soundbank during serialization.
#[derive(Debug)]
pub struct ObjectSourceFields<'bank> {
    bank: &'bank Bank,
    index: usize,
    family: SourceFamily,
}

impl<'bank> ObjectSourceFields<'bank> {
    pub(super) const fn new(bank: &'bank Bank, index: usize, kind: HircKind) -> Option<Self> {
        let family = match kind {
            HircKind::Action => SourceFamily::Action,
            HircKind::State => SourceFamily::State,
            HircKind::DialogueEvent => SourceFamily::Dialogue,
            HircKind::Attenuation => SourceFamily::Attenuation,
            HircKind::RandomSequenceContainer => SourceFamily::RandomSequence,
            HircKind::SwitchContainer => SourceFamily::Switch,
            HircKind::LayerContainer => SourceFamily::Blend,
            HircKind::MusicTrack => SourceFamily::MusicTrack,
            HircKind::FxShareSet | HircKind::FxCustom | HircKind::AudioDevice => {
                SourceFamily::Plugin(kind)
            }
            HircKind::Lfo | HircKind::Envelope | HircKind::TimeModulator => {
                SourceFamily::Modulator(kind)
            }
            _ => return None,
        };
        Some(Self {
            bank,
            index,
            family,
        })
    }
}

impl Serialize for ObjectSourceFields<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let origins = ObjectOrigins::new(self.bank, self.family).map_err(S::Error::custom)?;
        self.bank
            .soundbank()
            .with_hirc_source_fields(self.index, |header, observed| {
                project(serializer, &origins, &header, self.family, observed)
            })
            .map_err(S::Error::custom)?
            .ok_or_else(|| S::Error::custom("the selected original object is absent"))?
    }
}

fn project<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    family: SourceFamily,
    observed: BankHircSemantic<'_, R>,
) -> Result<S::Ok, S::Error> {
    match (observed, family) {
        (BankHircSemantic::ActionWithProperties(action), SourceFamily::Action) => {
            action::serialize(serializer, origins, header, *action)
        }
        (BankHircSemantic::Action(action), SourceFamily::Action) => {
            action::unobserved(serializer, origins, header, &action)
        }
        (BankHircSemantic::State(state), SourceFamily::State) => standalone_state::serialize(
            serializer,
            origins,
            header,
            &state.object,
            Some(&state.status),
        ),
        (BankHircSemantic::DialogueEvent(dialogue), SourceFamily::Dialogue) => {
            serialize_dialogue(serializer, origins, header, *dialogue)
        }
        (BankHircSemantic::Attenuation(attenuation), SourceFamily::Attenuation) => {
            attenuation::serialize(serializer, origins, header, *attenuation)
        }
        (BankHircSemantic::ContainerWithNode(container), SourceFamily::RandomSequence) => {
            random_sequence::serialize(serializer, origins, header, *container)
        }
        (BankHircSemantic::ContainerWithNode(container), SourceFamily::Switch) => {
            switch::serialize(serializer, origins, header, *container)
        }
        (BankHircSemantic::ContainerWithNode(container), SourceFamily::Blend) => {
            blend::serialize(serializer, origins, header, *container)
        }
        (BankHircSemantic::MusicTrack(track), SourceFamily::MusicTrack) => {
            music_track::serialize(serializer, origins, header, *track)
        }
        (BankHircSemantic::Plugin(plugin), SourceFamily::Plugin(_)) => {
            effect_source::plugin::serialize(serializer, origins, header, *plugin)
        }
        (BankHircSemantic::Modulator(modulator), SourceFamily::Modulator(_)) => {
            effect_source::modulator::serialize(serializer, origins, header, *modulator)
        }
        (BankHircSemantic::Raw(object), family)
            if object.kind(header.header().version) == family.kind() =>
        {
            project_raw(serializer, origins, header, family, &object)
        }
        _ => Err(S::Error::custom(
            "the selected original object has an unexpected source layout",
        )),
    }
}

fn project_raw<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    family: SourceFamily,
    object: &BankHircObject,
) -> Result<S::Ok, S::Error> {
    match family {
        SourceFamily::Action => action::unsupported(serializer, origins, header, object),
        SourceFamily::State => {
            standalone_state::serialize(serializer, origins, header, object, None)
        }
        SourceFamily::Dialogue => serialize_unsupported(serializer, origins, header, object),
        SourceFamily::Attenuation => attenuation::unsupported(serializer, origins, header, object),
        SourceFamily::RandomSequence => {
            random_sequence::unsupported(serializer, origins, header, object)
        }
        SourceFamily::Switch => switch::unsupported(serializer, origins, header, object),
        SourceFamily::Blend => blend::unsupported(serializer, origins, header, object),
        SourceFamily::MusicTrack => music_track::unsupported(serializer, origins, header, object),
        SourceFamily::Plugin(_) => {
            effect_source::plugin::unsupported(serializer, origins, header, object)
        }
        SourceFamily::Modulator(_) => {
            effect_source::modulator::unsupported(serializer, origins, header, object)
        }
    }
}

fn serialize_dialogue<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    dialogue: BankHircDialogueEvent<'_, R>,
) -> Result<S::Ok, S::Error> {
    let original = dialogue.object();
    let error = dialogue.format_error();
    let layout = if error.is_some() {
        DialogueSourceLayout::Malformed
    } else {
        DialogueSourceLayout::Decoded
    };
    let error = error
        .map(|error| origins.coverage(error.to_string(), header, original))
        .transpose()
        .map_err(S::Error::custom)?;
    let layout = origins
        .coverage(layout, header, original)
        .map_err(S::Error::custom)?;
    let fields = summary::collect(&dialogue, origins).map_err(S::Error::custom)?;
    DialogueSourceDocument {
        kind: DialogueSourceKind::DialogueEvent,
        layout,
        version: origins.version(header).map_err(S::Error::custom)?,
        error,
        fields,
        rows: DialogueRows::new(dialogue, origins),
        source_inventory: origins,
    }
    .serialize(serializer)
}

fn serialize_unsupported<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    object: &BankHircObject,
) -> Result<S::Ok, S::Error> {
    DialogueSourceDocument {
        kind: DialogueSourceKind::DialogueEvent,
        layout: origins
            .coverage(DialogueSourceLayout::Unsupported, header, object)
            .map_err(S::Error::custom)?,
        version: origins.version(header).map_err(S::Error::custom)?,
        error: None,
        fields: DialogueSummary::default(),
        rows: [] as [DialogueSourceRow; 0],
        source_inventory: origins,
    }
    .serialize(serializer)
}
