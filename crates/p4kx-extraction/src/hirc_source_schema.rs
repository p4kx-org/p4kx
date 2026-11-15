// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator};

use super::action::ActionSourceSchema;
use super::attenuation::AttenuationSourceSchema;
use super::blend::BlendSourceSchema;
use super::dto::DialogueSourceSchema;
use super::effect_source::modulator::ModulatorSourceSchema;
use super::effect_source::plugin::PluginSourceSchema;
use super::music_node::MusicNodeSourceSchema;
use super::music_track::MusicTrackSourceSchema;
use super::random_sequence::RandomSequenceSchema;
use super::standalone_state::StateSourceSchema;
use super::switch::SwitchSourceSchema;

pub(in crate::hirc_view) struct ObjectSourceSchema;

impl JsonSchema for ObjectSourceSchema {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("HircObjectSourceFields")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let alternatives = [
            generator.subschema_for::<DialogueSourceSchema>(),
            generator.subschema_for::<AttenuationSourceSchema>(),
            generator.subschema_for::<PluginSourceSchema>(),
            generator.subschema_for::<ModulatorSourceSchema>(),
            generator.subschema_for::<RandomSequenceSchema>(),
            generator.subschema_for::<SwitchSourceSchema>(),
            generator.subschema_for::<BlendSourceSchema>(),
            generator.subschema_for::<MusicTrackSourceSchema>(),
            generator.subschema_for::<MusicNodeSourceSchema>(),
            generator.subschema_for::<ActionSourceSchema>(),
            generator.subschema_for::<StateSourceSchema>(),
        ];
        schemars::json_schema!({ "anyOf": alternatives })
    }
}
