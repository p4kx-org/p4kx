// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::audio_bank_node_aux;
use crate::audio_bank_node_declaration_payload::NodeDeclarationPayload;
use crate::audio_bank_node_fx::{FX_NAMESPACE, SLOT_NAMESPACE};
use crate::{MediaError, schema_merge};

use super::{Binding, StatusRule};

const PAYLOAD: &str = "NodeDeclarationPayload";

pub(super) fn insert_payload(base: &mut Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        PAYLOAD,
        schema_merge::serialized_payload::<NodeDeclarationPayload<'static>>(),
    )
}

pub(super) const fn bindings() -> [Binding; 3] {
    [
        Binding::exact(
            PAYLOAD,
            FX_NAMESPACE,
            "bank-hirc-node-fx",
            3,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            SLOT_NAMESPACE,
            "bank-hirc-node-fx-slot",
            4,
            StatusRule::MalformedLayout,
        ),
        Binding::exact(
            PAYLOAD,
            audio_bank_node_aux::NAMESPACE,
            "bank-hirc-node-aux",
            2,
            StatusRule::MalformedLayout,
        ),
    ]
}
