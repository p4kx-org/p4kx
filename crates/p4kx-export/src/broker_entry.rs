// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::broker_reputation_read::BrokerReputationResult;
use crate::broker_standing_read::BrokerStandingTerm;
use crate::mission_flow_owner::MissionFlowOwnerRef;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;
use crate::template_projection::MissionContext;

/// Context computed from the single canonical source arena of a physical Broker.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BrokerProjection<'source> {
    pub(crate) broker: MissionFlowOwnerRef,
    pub(crate) title: Option<SourceValueRef>,
    #[serde(flatten)]
    pub(crate) context: MissionContext<'source>,
    pub(crate) standing_requirements: Vec<BrokerStandingTerm>,
    pub(crate) reputation_results: Vec<BrokerReputationResult>,
}
