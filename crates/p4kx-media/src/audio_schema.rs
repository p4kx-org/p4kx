// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_failure::FAILURE_NAMESPACE as BANK_FAILURE;
use crate::audio_bank_hirc::{HIRC_NAMESPACE, OBJECT_NAMESPACE};
use crate::audio_bank_hirc_action::ActionPayload;
use crate::audio_bank_hirc_container::ContainerPayload;
use crate::audio_bank_hirc_event::{self, EventPayload};
use crate::audio_bank_hirc_sound::SoundPayload;
use crate::audio_bank_media::MEDIA_NAMESPACE;
use crate::audio_bank_payload::BankPayload;
use crate::audio_bank_projection::{HEADER_NAMESPACE, SECTION_NAMESPACE};
use crate::audio_bank_stid::{NAME_NAMESPACE, STID_NAMESPACE, STID_TAIL_NAMESPACE};
use crate::audio_bank_stid_payload::BankStidPayload;
use crate::audio_loose_inventory::{self, LooseInventoryPayload};
use crate::audio_payload::AudioPayload;
use crate::audio_schema_status::StatusRule;
use crate::audio_source_language::{self, AudioSourceLanguage};
use crate::audio_wem_annotation_payload::AnnotationPayload;
use crate::audio_wem_diagnostic::OPAQUE_NAMESPACE;
use crate::audio_wem_projection::{EMBEDDED_INDEX_COUNT, embedded_namespace};
use crate::{MediaError, schema_merge};

#[path = "audio_schema_action_properties.rs"]
mod action_properties;
#[path = "audio_schema_actor_mixer.rs"]
mod actor_mixer;
#[path = "audio_schema_foreign.rs"]
mod foreign;
#[path = "audio_schema_hirc.rs"]
mod hirc;
#[path = "audio_schema_music.rs"]
mod music;
#[path = "audio_schema_node_declarations.rs"]
mod node_declarations;
#[path = "audio_schema_node_playback.rs"]
mod node_playback;
#[path = "audio_schema_node_properties.rs"]
mod node_properties;
#[path = "audio_schema_random_sequence.rs"]
mod random_sequence;
#[path = "audio_schema_rtpc.rs"]
mod rtpc;
#[path = "audio_schema_state.rs"]
mod state;
#[path = "audio_schema_switch_container.rs"]
mod switch_container;
#[path = "audio_schema_wem.rs"]
mod wem;

const LOOSE_INVENTORY: &str = "LooseInventoryPayload";
const SOURCE_LANGUAGE: &str = "AudioSourceLanguage";
const WEM_PAYLOAD: &str = "AudioPayload";
const ANNOTATION_PAYLOAD: &str = "AnnotationPayload";
const BANK_PAYLOAD: &str = "BankPayload";
const ACTION_PAYLOAD: &str = "ActionPayload";
const EVENT_PAYLOAD: &str = "EventPayload";
const SOUND_PAYLOAD: &str = "SoundPayload";
const CONTAINER_PAYLOAD: &str = "ContainerPayload";
const STID_PAYLOAD: &str = "BankStidPayload";

struct Binding {
    schema: &'static str,
    namespace: &'static str,
    record_type: &'static str,
    min_indices: u8,
    max_indices: u8,
    status: StatusRule,
}

impl Binding {
    const fn exact(
        schema: &'static str,
        namespace: &'static str,
        record_type: &'static str,
        indices: u8,
        status: StatusRule,
    ) -> Self {
        Self {
            schema,
            namespace,
            record_type,
            min_indices: indices,
            max_indices: indices,
            status,
        }
    }

    fn constraint(&self) -> Value {
        self.constraint_at(self.namespace, self.min_indices, self.max_indices)
    }

    fn embedded_constraint(&self) -> Result<Value, MediaError> {
        let namespace = embedded_namespace(self.namespace)?;
        Ok(self.constraint_at(
            &namespace,
            self.min_indices + EMBEDDED_INDEX_COUNT,
            self.max_indices + EMBEDDED_INDEX_COUNT,
        ))
    }

    fn constraint_at(&self, namespace: &str, min_indices: u8, max_indices: u8) -> Value {
        let Self {
            schema,
            record_type,
            status,
            ..
        } = self;
        let reference = format!("#/$defs/{schema}");
        serde_json::json!({
            "if": {
                "properties": {
                    "kind":{"const":"audio"},
                    "subresource":{"properties":{"namespace":{"const":namespace}},"required":["namespace"]}
                },
                "required":["kind","subresource"]
            },
            "then": {"allOf":[{
                "properties": {
                    "data":{"allOf":[
                        {"$ref":reference},
                        {"properties":{"record_type":{"const":record_type}},"required":["record_type"]}
                    ]},
                    "subresource":{"properties":{"indices":{"minItems":min_indices,"maxItems":max_indices}}}
                },
                "required":["data"]
            }, status.schema()]}
        })
    }
}

fn insert_audio_payloads(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        WEM_PAYLOAD,
        schema_merge::serialized_payload::<AudioPayload>(),
    )?;
    schema_merge::insert_payload(
        base,
        BANK_PAYLOAD,
        schema_merge::serialized_payload::<BankPayload>(),
    )?;
    schema_merge::insert_payload(
        base,
        STID_PAYLOAD,
        schema_merge::serialized_payload::<BankStidPayload<'static>>(),
    )?;
    schema_merge::insert_payload(
        base,
        ANNOTATION_PAYLOAD,
        schema_merge::serialized_payload::<AnnotationPayload<'static>>(),
    )?;
    schema_merge::insert_payload(
        base,
        ACTION_PAYLOAD,
        schema_merge::serialized_payload::<ActionPayload>(),
    )?;
    schema_merge::insert_payload(
        base,
        EVENT_PAYLOAD,
        schema_merge::serialized_payload::<EventPayload>(),
    )?;
    schema_merge::insert_payload(
        base,
        SOUND_PAYLOAD,
        schema_merge::serialized_payload::<SoundPayload>(),
    )?;
    schema_merge::insert_payload(
        base,
        CONTAINER_PAYLOAD,
        schema_merge::serialized_payload::<ContainerPayload>(),
    )?;
    schema_merge::insert_payload(
        base,
        SOURCE_LANGUAGE,
        schema_merge::serialized_payload::<AudioSourceLanguage<'static>>(),
    )?;
    schema_merge::insert_payload(
        base,
        LOOSE_INVENTORY,
        schema_merge::serialized_payload::<LooseInventoryPayload<'static>>(),
    )?;
    insert_hirc_field_payloads(base)?;
    foreign::insert_payload(base)
}

fn insert_hirc_field_payloads(base: &mut Value) -> Result<(), MediaError> {
    music::insert_payload(base)?;
    state::insert_payload(base)?;
    rtpc::insert_payload(base)?;
    actor_mixer::insert_payload(base)?;
    node_properties::insert_payload(base)?;
    node_declarations::insert_payload(base)?;
    node_playback::insert_payload(base)?;
    action_properties::insert_payload(base)?;
    random_sequence::insert_payload(base)?;
    switch_container::insert_payload(base)
}

pub(crate) fn bind_record_schema(mut base: Value) -> Result<Value, MediaError> {
    insert_audio_payloads(&mut base)?;
    let root = base
        .as_object_mut()
        .ok_or_else(|| MediaError::Contract("audio record schema is not an object".into()))?;
    let constraints = root
        .entry("allOf")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| MediaError::Contract("media record allOf is not an array".into()))?;
    constraints.extend(
        wem::bindings()
            .iter()
            .chain(std::iter::once(&SOURCE_LANGUAGE_BINDING))
            .chain(loose_bindings().iter())
            .chain(bank_bindings().iter())
            .chain(hirc::bindings().iter())
            .chain(music::bindings().iter())
            .chain(state::bindings().iter())
            .chain(rtpc::bindings().iter())
            .chain(actor_mixer::bindings().iter())
            .chain(node_properties::bindings().iter())
            .chain(node_declarations::bindings().iter())
            .chain(node_playback::bindings().iter())
            .chain(action_properties::bindings().iter())
            .chain(random_sequence::bindings().iter())
            .chain(switch_container::bindings().iter())
            .chain(foreign::bindings().iter())
            .chain(std::iter::once(&BANK_FAILURE_BINDING))
            .chain(stid_bindings().iter())
            .chain(event_bindings().iter())
            .chain(wem::annotation_bindings().iter())
            .map(Binding::constraint),
    );
    for binding in wem::bindings()
        .into_iter()
        .filter(|binding| binding.namespace != OPAQUE_NAMESPACE)
        .chain(wem::annotation_bindings())
    {
        constraints.push(binding.embedded_constraint()?);
    }
    Ok(base)
}

const fn bank_bindings() -> [Binding; 5] {
    [
        Binding::exact(
            BANK_PAYLOAD,
            HEADER_NAMESPACE,
            "bank",
            1,
            StatusRule::Complete,
        ),
        Binding::exact(
            BANK_PAYLOAD,
            SECTION_NAMESPACE,
            "bank-section",
            1,
            StatusRule::DuplicateSection,
        ),
        Binding::exact(
            BANK_PAYLOAD,
            MEDIA_NAMESPACE,
            "bank-media",
            2,
            StatusRule::MediaRange,
        ),
        Binding::exact(
            BANK_PAYLOAD,
            OBJECT_NAMESPACE,
            "bank-object",
            2,
            StatusRule::Complete,
        ),
        Binding::exact(
            BANK_PAYLOAD,
            HIRC_NAMESPACE,
            "bank-hirc",
            1,
            StatusRule::Complete,
        ),
    ]
}

const fn loose_bindings() -> [Binding; 2] {
    [
        Binding::exact(
            LOOSE_INVENTORY,
            audio_loose_inventory::CANDIDATE_NAMESPACE,
            audio_loose_inventory::CANDIDATE_RECORD,
            1,
            StatusRule::Complete,
        ),
        Binding::exact(
            LOOSE_INVENTORY,
            audio_loose_inventory::INVENTORY_NAMESPACE,
            audio_loose_inventory::INVENTORY_RECORD,
            0,
            StatusRule::LooseInventory,
        ),
    ]
}

const SOURCE_LANGUAGE_BINDING: Binding = Binding::exact(
    SOURCE_LANGUAGE,
    audio_source_language::NAMESPACE,
    audio_source_language::RECORD_TYPE,
    0,
    StatusRule::Complete,
);

const BANK_FAILURE_BINDING: Binding = Binding {
    schema: BANK_PAYLOAD,
    namespace: BANK_FAILURE,
    record_type: "bank-failure",
    min_indices: 1,
    max_indices: 2,
    status: StatusRule::Failed,
};

const fn event_bindings() -> [Binding; 5] {
    [
        Binding::exact(
            EVENT_PAYLOAD,
            audio_bank_hirc_event::EVENT_NAMESPACE,
            "bank-hirc-event",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            EVENT_PAYLOAD,
            audio_bank_hirc_event::ACTION_NAMESPACE,
            "bank-hirc-event-action",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            EVENT_PAYLOAD,
            audio_bank_hirc_event::RESOLUTION_NAMESPACE,
            "bank-hirc-event-action-resolution",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            EVENT_PAYLOAD,
            audio_bank_hirc_event::CANDIDATE_NAMESPACE,
            "bank-hirc-event-action-candidate",
            4,
            StatusRule::Complete,
        ),
        Binding::exact(
            EVENT_PAYLOAD,
            audio_bank_hirc_event::TAIL_NAMESPACE,
            "bank-hirc-event-tail",
            2,
            StatusRule::Complete,
        ),
    ]
}

const fn stid_bindings() -> [Binding; 3] {
    [
        Binding::exact(
            STID_PAYLOAD,
            STID_NAMESPACE,
            "bank-stid",
            1,
            StatusRule::Complete,
        ),
        Binding::exact(
            STID_PAYLOAD,
            NAME_NAMESPACE,
            "bank-name",
            2,
            StatusRule::Complete,
        ),
        Binding::exact(
            STID_PAYLOAD,
            STID_TAIL_NAMESPACE,
            "bank-stid-tail",
            1,
            StatusRule::Complete,
        ),
    ]
}
