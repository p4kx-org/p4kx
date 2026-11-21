// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::origins;
use super::super::payload::{self as wire, DefinitionPayload};
use super::join::Match;
use super::{Alias, AliasKind, Site, Target};
use crate::audio_record_origins as records;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, Status, Subresource};

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct ClipContributor {
    placement_ordinal: DecimalU64,
    #[serde(skip_serializing_if = "Option::is_none")]
    database_binding_ordinal: Option<DecimalU64>,
    clip: ClipSite,
    alias: AliasSite,
    target: ContributorTarget,
    match_basis: MatchBasis,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ClipContributorClipSite")]
struct ClipSite {
    entry_index: DecimalU64,
    element_ordinal: DecimalU64,
    declaration_ordinal: DecimalU64,
}

impl From<Site> for ClipSite {
    fn from(site: Site) -> Self {
        let [element, declaration] = site.indices;
        Self {
            entry_index: DecimalU64::new(site.entry as u64),
            element_ordinal: DecimalU64::new(element),
            declaration_ordinal: DecimalU64::new(declaration),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ClipContributorAliasSite")]
struct AliasSite {
    entry_index: DecimalU64,
    element_ordinal: DecimalU64,
    declaration_ordinal: DecimalU64,
    kind: AliasKind,
    candidate_ordinal: DecimalU64,
}

impl From<&Alias> for AliasSite {
    fn from(alias: &Alias) -> Self {
        let [element, declaration] = alias.site.indices;
        Self {
            entry_index: DecimalU64::new(alias.site.entry as u64),
            element_ordinal: DecimalU64::new(element),
            declaration_ordinal: DecimalU64::new(declaration),
            kind: alias.kind,
            candidate_ordinal: DecimalU64::new(alias.ordinal),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "ClipContributorTarget")]
enum ContributorTarget {
    Caf {
        entry_index: DecimalU64,
    },
    Dba {
        entry_index: DecimalU64,
        clip_ordinal: DecimalU64,
    },
    BlendSpace {
        entry_index: DecimalU64,
    },
    CombinedSpace {
        entry_index: DecimalU64,
    },
}

impl From<Target> for ContributorTarget {
    fn from(target: Target) -> Self {
        let ordinal = |entry| DecimalU64::new(entry as u64);
        match target {
            Target::Caf { entry } => Self::Caf {
                entry_index: ordinal(entry),
            },
            Target::Dba { entry, clip } => Self::Dba {
                entry_index: ordinal(entry),
                clip_ordinal: DecimalU64::new(clip),
            },
            Target::BlendSpace { entry } => Self::BlendSpace {
                entry_index: ordinal(entry),
            },
            Target::CombinedSpace { entry } => Self::CombinedSpace {
                entry_index: ordinal(entry),
            },
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "ClipContributorMatchBasis")]
enum MatchBasis {
    Exact,
    AsciiCaseVariant,
}

pub(super) fn write(
    matched: &Match<'_>,
    join_proof: OriginContributions,
    sequence: u64,
    owner: &MediaOriginInput,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let prepared = prepare(matched, join_proof, owner, writer)?;
    let basis = if matched.clip.authored == matched.alias.name {
        MatchBasis::Exact
    } else {
        MatchBasis::AsciiCaseVariant
    };
    let contributor = ClipContributor {
        placement_ordinal: DecimalU64::new(matched.placement.ordinal),
        database_binding_ordinal: matched
            .binding
            .map(|binding| DecimalU64::new(binding.ordinal)),
        clip: matched.clip.site.into(),
        alias: matched.alias.into(),
        target: matched.alias.target.into(),
        match_basis: basis,
    };
    prepared.write(
        writer,
        &Subresource::new(
            wire::CLIP_CONTRIBUTOR,
            vec![matched.placement.ordinal, sequence],
        ),
        Status::Complete,
        &DefinitionPayload::ClipContributor(contributor),
        &[],
    )
}

fn prepare(
    matched: &Match<'_>,
    join_proof: OriginContributions,
    owner: &MediaOriginInput,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = origins::record(owner, join_proof, writer)?;
    let mut basis_proof = writer.capture_origins();
    basis_proof.include(&matched.clip.proof, writer)?;
    basis_proof.include(&matched.alias.name_proof, writer)?;
    records::field(
        &mut prepared,
        "/data/payload/match_basis",
        basis_proof.finish(),
        writer,
    )?;
    records::field(
        &mut prepared,
        "/data/payload/placement_ordinal",
        matched.placement.ordinal_proof.clone(),
        writer,
    )?;
    if let Some(binding) = matched.binding {
        records::field(
            &mut prepared,
            "/data/payload/database_binding_ordinal",
            binding.proof.clone(),
            writer,
        )?;
    }
    for (field, source) in [
        ("/data/payload/clip", &matched.clip.proof),
        ("/data/payload/alias", &matched.alias.proof),
        ("/data/payload/target", &matched.alias.target_proof),
    ] {
        records::subtree(&mut prepared, field, source.clone(), writer)?;
    }
    for field in ["entry_index", "element_ordinal", "declaration_ordinal"] {
        records::field(
            &mut prepared,
            &format!("/data/payload/alias/{field}"),
            matched.alias.site_proof.clone(),
            writer,
        )?;
    }
    if let Some(proof) = &matched.alias.target_clip_proof {
        records::field(
            &mut prepared,
            "/data/payload/target/clip_ordinal",
            proof.clone(),
            writer,
        )?;
    }
    Ok(prepared)
}
