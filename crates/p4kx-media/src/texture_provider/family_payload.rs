// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, TextureMember, TextureMemberRelation, TextureMemberRole};
use schemars::JsonSchema;
use serde::Serialize;

use crate::{CandidateState, ReferenceUnavailable};

pub(super) const MEMBER: &str = "texture.dds-member";
pub(super) const EXPECTED: &str = "texture.dds-expected-part";
pub(super) const FAMILY: &str = "texture.dds-family";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", rename_all = "kebab-case")]
pub(super) enum FamilyPayload {
    Member(Member),
    ExpectedPart(ExpectedPart),
    Family(Family),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "TextureDdsMember")]
pub(super) struct Member {
    pub(super) entry: MemberIdentity,
    pub(super) relation: Relation,
    pub(super) selection: CandidateState,
    pub(super) filter: MemberFilter,
    pub(super) declared_bytes: DecimalU64,
    pub(super) payload_access: PayloadAccess,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "TextureDdsMemberIdentity")]
pub(super) struct MemberIdentity {
    pub(super) entry_index: usize,
    pub(super) path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) raw_path_hex: Option<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "TextureDdsMemberFilter")]
pub(super) enum MemberFilter {
    Included,
    Unmatched,
    Excluded,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(tag = "role", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "TextureDdsRelation")]
pub(super) enum Relation {
    ColorHeader,
    ColorPart { number: u32 },
    AlphaHeader,
    AlphaPart { number: u32 },
    UnprovedPath,
}

impl Relation {
    pub(super) const fn of(relation: TextureMemberRelation<'_>, lossless_owner: bool) -> Self {
        if !lossless_owner {
            return Self::UnprovedPath;
        }
        match relation {
            TextureMemberRelation::Matched {
                role: TextureMemberRole::Header,
                ..
            } => Self::ColorHeader,
            TextureMemberRelation::Matched {
                role: TextureMemberRole::Member(_, member),
                ..
            } => match member {
                TextureMember::Part(number) => Self::ColorPart { number },
                TextureMember::AlphaHeader => Self::AlphaHeader,
                TextureMember::AlphaPart(number) => Self::AlphaPart { number },
                _ => Self::UnprovedPath,
            },
            _ => Self::UnprovedPath,
        }
    }

    pub(super) const fn number(self, alpha: bool) -> Option<u32> {
        match (self, alpha) {
            (Self::ColorPart { number }, false) | (Self::AlphaPart { number }, true) => {
                Some(number)
            }
            _ => None,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "TextureMemberPayloadAccess")]
pub(super) enum PayloadAccess {
    NotInspected,
    KeyRequired,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "TextureDdsExpectedPart")]
pub(super) struct ExpectedPart {
    pub(super) number: u32,
    pub(super) level: u32,
    pub(super) expected_bytes: DecimalU64,
    pub(super) matching_members: DecimalU64,
    pub(super) disposition: PartDisposition,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "TextureDdsPartDisposition")]
pub(super) enum PartDisposition {
    Missing,
    DeclaredSizeMatches,
    DeclaredSizeMismatch,
    Ambiguous,
    Unproved,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "TextureDdsFamily")]
pub(super) struct Family {
    pub(super) observed_members: DecimalU64,
    pub(super) directory: Directory,
    pub(super) member_paths: MemberPaths,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "TextureDdsDirectory")]
pub(super) enum Directory {
    Complete,
    Incomplete { reason: ReferenceUnavailable },
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "TextureDdsMemberPaths")]
pub(super) enum MemberPaths {
    Lossless,
    Unproved,
}
