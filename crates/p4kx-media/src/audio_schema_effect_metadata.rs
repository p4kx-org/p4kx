// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_hirc::effect_metadata::payload::{
    BANK_DATA, MODULATOR, MODULATOR_PROPERTY, MetadataPayload, OVERRIDE, PLUGIN,
};
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const PAYLOAD: &str = "EffectMetadataPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        PAYLOAD,
        schema_merge::serialized_payload::<MetadataPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 5] {
    [
        Binding::exact(
            PAYLOAD,
            PLUGIN,
            "bank-hirc-plugin",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            BANK_DATA,
            "bank-hirc-plugin-bank-data",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            PAYLOAD,
            OVERRIDE,
            "bank-hirc-plugin-state-override",
            3,
            StatusRule::Complete,
        ),
        Binding::exact(
            PAYLOAD,
            MODULATOR,
            "bank-hirc-modulator",
            2,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            MODULATOR_PROPERTY,
            "bank-hirc-modulator-property",
            4,
            StatusRule::Complete,
        ),
    ]
}
