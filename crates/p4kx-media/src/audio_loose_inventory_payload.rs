// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Serialize;

use crate::{CandidateState, ContainerIdentity};

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum LooseInventoryPayload<'p> {
    LooseWemCandidate(Candidate<'p>),
    LooseWemInventory(Inventory),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Candidate<'p> {
    #[schemars(schema_with = "serialized_entry")]
    pub(super) entry: ContainerIdentity,
    pub(super) media_id: u32,
    pub(super) placement: Placement<'p>,
    pub(super) selection: CandidateState,
    pub(super) payload_access: PayloadAccess,
}

fn serialized_entry(generator: &mut SchemaGenerator) -> Schema {
    ContainerIdentity::json_schema(generator)
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Placement<'p> {
    Neutral,
    Language { folder: &'p str },
    Unavailable,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PayloadAccess {
    KeyRequired,
    NotInspected,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Inventory {
    pub(super) observed_candidates: DecimalU64,
    pub(super) candidates_complete: bool,
}
