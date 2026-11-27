// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_node_declaration_payload::{NodeDeclaration, NodeReferenceStatus};
use crate::audio_bank_payload::{BankEntryRange, BankField};

pub(crate) const SUMMARY_NAMESPACE: &str = "audio-bank-hirc-switch-container";
pub(crate) const GROUP_NAMESPACE: &str = "audio-bank-hirc-switch-membership-group";
pub(crate) const MEMBER_NAMESPACE: &str = "audio-bank-hirc-switch-membership";
pub(crate) const CONFIG_NAMESPACE: &str = "audio-bank-hirc-switch-child-configuration";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum SwitchContainerPayload<'row> {
    #[serde(rename = "bank-hirc-switch-container")]
    Summary(&'row NodeDeclaration<SwitchContainerObservations>),
    #[serde(rename = "bank-hirc-switch-membership-group")]
    Group(&'row SwitchMembershipGroupRecord),
    #[serde(rename = "bank-hirc-switch-membership")]
    Member(&'row SwitchMemberRecord),
    #[serde(rename = "bank-hirc-switch-child-configuration")]
    Configuration(&'row SwitchConfigurationRecord),
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SwitchContainerObservations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) group_type: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) group_kind: Option<SwitchGroupKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) group_id: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) default_value_id: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) continuous_validation: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) groups: Option<SwitchTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) configurations: Option<SwitchTable>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SwitchGroupKind {
    Switch,
    State,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SwitchTable {
    pub(crate) declared_count: BankField<u32>,
    pub(crate) checked_range: BankEntryRange,
    pub(crate) coverage: SwitchTableCoverage,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SwitchTableCoverage {
    Complete,
    Truncated,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SwitchMembershipGroupRecord {
    Decoded {
        value_id: BankField<u32>,
        member_count: BankField<u32>,
        members_range: BankEntryRange,
    },
    Malformed {
        value_id: BankField<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        member_count: Option<BankField<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        members_range: Option<BankEntryRange>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SwitchMemberRecord {
    pub(crate) child_id: BankField<u32>,
    pub(crate) reference_resolution: NodeReferenceStatus,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SwitchConfigurationRecord {
    Decoded {
        child_id: BankField<u32>,
        flags: BankField<u8>,
        fade_out: SwitchFadeWord,
        fade_in: SwitchFadeWord,
        reference_resolution: NodeReferenceStatus,
    },
    Malformed {
        child_id: BankField<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        flags: Option<BankField<u8>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_out: Option<SwitchFadeWord>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_in: Option<SwitchFadeWord>,
        reference_resolution: NodeReferenceStatus,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SwitchFadeWord {
    pub(crate) bits: BankField<u32>,
    pub(crate) signed_value: i32,
}
