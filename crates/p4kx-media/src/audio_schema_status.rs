// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::Status;

#[derive(Clone, Copy)]
pub(crate) enum StatusRule {
    Complete,
    Failed,
    DuplicateSection,
    MediaRange,
    MalformedLayout,
    Conversion,
    ShortListItem,
}

impl StatusRule {
    pub(crate) fn schema(self) -> Value {
        match self {
            Self::Complete => status_schema(Status::Complete),
            Self::Failed => status_schema(Status::Failed),
            Self::DuplicateSection => partial_if(
                &["data", "payload", "duplicate_unique"],
                &serde_json::json!(true),
            ),
            Self::MediaRange => partial_if(
                &["data", "payload", "data", "status"],
                &serde_json::json!("outside-data"),
            ),
            Self::MalformedLayout => partial_if(
                &["data", "payload", "layout"],
                &serde_json::json!("malformed"),
            ),
            Self::Conversion => partial_if(
                &["data", "payload", "outcome", "type"],
                &serde_json::json!("failed"),
            ),
            Self::ShortListItem => partial_if(
                &["data", "payload", "content", "type"],
                &serde_json::json!("malformed"),
            ),
        }
    }
}

fn partial_if(path: &[&str], value: &Value) -> Value {
    let predicate = path.iter().rev().fold(
        serde_json::json!({"const":value}),
        |property, name| serde_json::json!({"properties":{(*name):property},"required":[name]}),
    );
    serde_json::json!({
        "if": predicate,
        "then": status_schema(Status::Partial),
        "else": status_schema(Status::Complete)
    })
}

fn status_schema(status: Status) -> Value {
    let issues = match status {
        Status::Failed => {
            serde_json::json!({"contains":{"properties":{"severity":{"const":"error"}},"required":["severity"]}})
        }
        Status::Partial => {
            serde_json::json!({"contains":{"properties":{"severity":{"enum":["warning","error"]}},"required":["severity"]}})
        }
        Status::Complete => {
            serde_json::json!({"items":{"properties":{"severity":{"const":"info"}},"required":["severity"]}})
        }
    };
    serde_json::json!({"properties":{"status":{"const":status},"issues":issues}})
}
