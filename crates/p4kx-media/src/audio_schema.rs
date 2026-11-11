// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_failure::FAILURE_NAMESPACE as BANK_FAILURE;
use crate::audio_bank_hirc::{HIRC_NAMESPACE, OBJECT_NAMESPACE};
use crate::audio_bank_hirc_action::{self, ActionPayload};
use crate::audio_bank_hirc_event::{self, EventPayload};
use crate::audio_bank_hirc_sound::{self, SoundPayload};
use crate::audio_bank_media::MEDIA_NAMESPACE;
use crate::audio_bank_payload::BankPayload;
use crate::audio_bank_projection::{HEADER_NAMESPACE, SECTION_NAMESPACE};
use crate::audio_bank_stid::{NAME_NAMESPACE, STID_NAMESPACE, STID_TAIL_NAMESPACE};
use crate::audio_bank_stid_payload::BankStidPayload;
use crate::audio_payload::AudioPayload;
use crate::audio_schema_status::StatusRule;
use crate::audio_wem_annotation::FAILURE_NAMESPACE as ANNOTATION_FAILURE;
use crate::audio_wem_annotation_payload::AnnotationPayload;
use crate::audio_wem_cue::{CUE_NAMESPACE, POINT_NAMESPACE, TAIL_NAMESPACE};
use crate::audio_wem_diagnostic::{FAILURE_NAMESPACE as WEM_FAILURE, OPAQUE_NAMESPACE};
use crate::audio_wem_list::{ITEM_NAMESPACE, LIST_NAMESPACE};
use crate::audio_wem_projection::{
    CHUNK_NAMESPACE, EMBEDDED_INDEX_COUNT, WEM_NAMESPACE, embedded_namespace,
};
use crate::{MediaError, schema_merge};

const WEM_PAYLOAD: &str = "AudioPayload";
const ANNOTATION_PAYLOAD: &str = "AnnotationPayload";
const BANK_PAYLOAD: &str = "BankPayload";
const ACTION_PAYLOAD: &str = "ActionPayload";
const EVENT_PAYLOAD: &str = "EventPayload";
const SOUND_PAYLOAD: &str = "SoundPayload";
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

pub(crate) fn bind_record_schema(mut base: Value) -> Result<Value, MediaError> {
    schema_merge::insert_payload(
        &mut base,
        WEM_PAYLOAD,
        schema_merge::serialized_payload::<AudioPayload>(),
    )?;
    schema_merge::insert_payload(
        &mut base,
        BANK_PAYLOAD,
        schema_merge::serialized_payload::<BankPayload>(),
    )?;
    schema_merge::insert_payload(
        &mut base,
        STID_PAYLOAD,
        schema_merge::serialized_payload::<BankStidPayload<'static>>(),
    )?;
    schema_merge::insert_payload(
        &mut base,
        ANNOTATION_PAYLOAD,
        schema_merge::serialized_payload::<AnnotationPayload<'static>>(),
    )?;
    schema_merge::insert_payload(
        &mut base,
        ACTION_PAYLOAD,
        schema_merge::serialized_payload::<ActionPayload>(),
    )?;
    schema_merge::insert_payload(
        &mut base,
        EVENT_PAYLOAD,
        schema_merge::serialized_payload::<EventPayload>(),
    )?;
    schema_merge::insert_payload(
        &mut base,
        SOUND_PAYLOAD,
        schema_merge::serialized_payload::<SoundPayload>(),
    )?;
    let root = base
        .as_object_mut()
        .ok_or_else(|| MediaError::Contract("audio record schema is not an object".into()))?;
    let constraints = root
        .entry("allOf")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| MediaError::Contract("media record allOf is not an array".into()))?;
    constraints.extend(
        wem_bindings()
            .iter()
            .chain(bank_bindings().iter())
            .chain(stid_bindings().iter())
            .chain(event_bindings().iter())
            .chain(annotation_bindings().iter())
            .map(Binding::constraint),
    );
    for binding in wem_bindings()
        .into_iter()
        .filter(|binding| binding.namespace != OPAQUE_NAMESPACE)
        .chain(annotation_bindings())
    {
        constraints.push(binding.embedded_constraint()?);
    }
    Ok(base)
}

const fn wem_bindings() -> [Binding; 4] {
    [
        Binding::exact(WEM_PAYLOAD, WEM_NAMESPACE, "wem", 0, StatusRule::Complete),
        Binding::exact(
            WEM_PAYLOAD,
            CHUNK_NAMESPACE,
            "wem-chunk",
            1,
            StatusRule::Complete,
        ),
        Binding::exact(
            WEM_PAYLOAD,
            OPAQUE_NAMESPACE,
            "wem-opaque",
            0,
            StatusRule::Complete,
        ),
        Binding::exact(
            WEM_PAYLOAD,
            WEM_FAILURE,
            "wem-failure",
            1,
            StatusRule::Failed,
        ),
    ]
}

const fn bank_bindings() -> [Binding; 8] {
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
        Binding::exact(
            ACTION_PAYLOAD,
            audio_bank_hirc_action::NAMESPACE,
            "bank-hirc-action",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            SOUND_PAYLOAD,
            audio_bank_hirc_sound::NAMESPACE,
            "bank-hirc-sound",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding {
            schema: BANK_PAYLOAD,
            namespace: BANK_FAILURE,
            record_type: "bank-failure",
            min_indices: 1,
            max_indices: 2,
            status: StatusRule::Failed,
        },
    ]
}

const fn event_bindings() -> [Binding; 3] {
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

const fn annotation_bindings() -> [Binding; 6] {
    [
        Binding::exact(
            ANNOTATION_PAYLOAD,
            CUE_NAMESPACE,
            "wem-cue",
            1,
            StatusRule::Complete,
        ),
        Binding::exact(
            ANNOTATION_PAYLOAD,
            POINT_NAMESPACE,
            "wem-cue-point",
            2,
            StatusRule::Complete,
        ),
        Binding::exact(
            ANNOTATION_PAYLOAD,
            TAIL_NAMESPACE,
            "wem-cue-tail",
            1,
            StatusRule::Complete,
        ),
        Binding::exact(
            ANNOTATION_PAYLOAD,
            LIST_NAMESPACE,
            "wem-list",
            1,
            StatusRule::Complete,
        ),
        Binding::exact(
            ANNOTATION_PAYLOAD,
            ITEM_NAMESPACE,
            "wem-list-item",
            2,
            StatusRule::ShortListItem,
        ),
        Binding::exact(
            ANNOTATION_PAYLOAD,
            ANNOTATION_FAILURE,
            "wem-annotation-failure",
            1,
            StatusRule::Failed,
        ),
    ]
}
