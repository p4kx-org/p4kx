// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_random_sequence_payload::{
    ITEM_NAMESPACE, RandomSequencePayload, SUMMARY_NAMESPACE,
};
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const PAYLOAD: &str = "RandomSequencePayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        PAYLOAD,
        schema_merge::serialized_payload::<RandomSequencePayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 2] {
    [
        Binding::exact(
            PAYLOAD,
            SUMMARY_NAMESPACE,
            "bank-hirc-random-sequence",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            ITEM_NAMESPACE,
            "bank-hirc-random-sequence-playlist-item",
            3,
            StatusRule::MalformedLayout,
        ),
    ]
}
