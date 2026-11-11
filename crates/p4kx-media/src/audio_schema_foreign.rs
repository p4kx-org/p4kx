// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_foreign::rows::{self, ForeignPayload};
use crate::audio_schema_status::StatusRule;
use crate::{MediaError, schema_merge};

use super::Binding;

const PAYLOAD: &str = "ForeignPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        PAYLOAD,
        schema_merge::serialized_payload::<ForeignPayload>(),
    )
}

pub(super) const fn bindings() -> [Binding; 3] {
    [
        Binding::exact(
            PAYLOAD,
            rows::SEARCH,
            "bank-foreign-target-resolution",
            1,
            StatusRule::Complete,
        ),
        Binding::exact(
            PAYLOAD,
            rows::CANDIDATE,
            "bank-foreign-target-candidate",
            2,
            StatusRule::Complete,
        ),
        Binding::exact(
            PAYLOAD,
            rows::BLOCKED,
            "bank-foreign-target-blocker",
            2,
            StatusRule::Complete,
        ),
    ]
}
