// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    MissionInputTags, MissionModuleInput, MissionModuleInputPayload,
};
use serde_json::{Value, json};

use super::{TestResult, proof::View};

impl View<'_> {
    pub(super) fn input(&mut self, value: &MissionModuleInput<'_>) -> TestResult<Value> {
        let mut row = json!({
            "sourceType": self.scalar(value.source_type())?,
            "name": self.scalar(value.name())?,
            "authoredFields": self.arena(value.authored_fields())?,
        });
        let Value::Object(payload) = self.input_payload(value.payload())? else {
            return Err("input payload object".into());
        };
        row.as_object_mut().ok_or("input object")?.extend(payload);
        Ok(row)
    }

    fn input_payload(&mut self, value: &MissionModuleInputPayload<'_>) -> TestResult<Value> {
        Ok(match value {
            MissionModuleInputPayload::Boolean { value } => {
                json!({"kind":"boolean", "value":self.scalar(value)?})
            }
            MissionModuleInputPayload::Integer { value } => {
                json!({"kind":"integer", "value":self.scalar(value)?})
            }
            MissionModuleInputPayload::Float { value } => {
                json!({"kind":"float", "value":self.float(value)?})
            }
            MissionModuleInputPayload::Activity { value } => {
                json!({"kind":"activity", "value":self.scalar(value)?})
            }
            MissionModuleInputPayload::Tag { value } => {
                json!({"kind":"tag", "value":self.cell(value, Self::input_tags)?})
            }
            MissionModuleInputPayload::Base => json!({"kind":"base"}),
            MissionModuleInputPayload::Authored => json!({"kind":"authored"}),
            _ => return Err("the consumer must deliberately handle a new input variant".into()),
        })
    }

    fn input_tags(&mut self, value: &MissionInputTags<'_>) -> TestResult<Value> {
        Ok(json!({
            "positiveTags": self.sequence(value.positive_tags(), |view, value| view.reference(value))?,
            "negativeTags": self.sequence(value.negative_tags(), |view, value| view.reference(value))?,
            "authoredFields": self.arena(value.authored_fields())?,
        }))
    }
}
