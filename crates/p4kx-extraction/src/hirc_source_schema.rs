// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator};

use super::blend::BlendSourceSchema;
use super::dto::DialogueSourceSchema;
use super::random_sequence::RandomSequenceSchema;
use super::switch::SwitchSourceSchema;

pub(in crate::hirc_view) struct ObjectSourceSchema;

impl JsonSchema for ObjectSourceSchema {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("HircObjectSourceFields")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let alternatives = [
            generator.subschema_for::<DialogueSourceSchema>(),
            generator.subschema_for::<RandomSequenceSchema>(),
            generator.subschema_for::<SwitchSourceSchema>(),
            generator.subschema_for::<BlendSourceSchema>(),
        ];
        schemars::json_schema!({ "anyOf": alternatives })
    }
}
