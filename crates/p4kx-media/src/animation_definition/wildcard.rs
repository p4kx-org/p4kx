// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::cryxml::{SourceAttribute, SourceElement};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::chrparams_selectors::AnimationAttributes;
use super::super::origins;
use super::super::payload::{self as wire, DefinitionPayload};
use super::super::projection::{DefinitionProjection, Failure};
use super::super::wildcard_expansion;
use super::super::wildcard_match::{Rule, Scope, Unexpanded};
use super::{Directive, combine};
use crate::audio_control_payload::AtlPosition;
use crate::audio_record_origins as records;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::xml_source_projection::{declaration_position, element_position};
use crate::{MediaWriter, Status, Subresource};

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Wildcard<'source> {
    state: WildcardState,
    name: Attribute<'source>,
    path: Attribute<'source>,
    preceding_filepath: FilepathScope<'source>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum WildcardState {
    Unexpanded,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Attribute<'source> {
    qualified_name: &'source str,
    value: &'source str,
    position: AtlPosition,
}

impl<'source> Attribute<'source> {
    fn from(attribute: &SourceAttribute<'source>) -> Result<Self, Failure> {
        Ok(Self {
            qualified_name: attribute.name(),
            value: attribute.value(),
            position: declaration_position(attribute.position())?,
        })
    }

    fn proof(
        &self,
        projection: &DefinitionProjection<'_, '_>,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, Failure> {
        Ok(origins::position(
            projection.input,
            self.position,
            SourceUseRole::Selector,
            writer,
        )?)
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
enum FilepathScope<'source> {
    Missing {
        list_position: AtlPosition,
    },
    Malformed {
        directive_position: AtlPosition,
    },
    Reset {
        directive_position: AtlPosition,
        path: Attribute<'source>,
    },
    Authored {
        directive_position: AtlPosition,
        path: Attribute<'source>,
    },
}

impl<'source> FilepathScope<'source> {
    const fn matching_scope(&self) -> Scope<'source> {
        match self {
            Self::Missing { .. } => Scope::Missing,
            Self::Malformed { .. } => Scope::Malformed,
            Self::Reset { .. } => Scope::Reset,
            Self::Authored { path, .. } => Scope::Authored(path.value),
        }
    }
}

pub(super) fn is_wildcard(name: &str, path: &str) -> bool {
    !name.is_empty()
        && !path.is_empty()
        && !name.starts_with(['#', '$'])
        && (name.contains(['*', '?']) || path.contains(['*', '?']))
}

pub(super) fn write(
    attributes: &AnimationAttributes<'_>,
    list: SourceElement<'_>,
    directive: Option<&Directive<'_>>,
    projection: &DefinitionProjection<'_, '_>,
    indices: [u64; 2],
    writer: &mut MediaWriter<'_>,
) -> Result<(), Failure> {
    let name = Attribute::from(&attributes.name)?;
    let path = Attribute::from(&attributes.path)?;
    let name_proof = name.proof(projection, writer)?;
    let path_proof = path.proof(projection, writer)?;
    let selector = combine([&name_proof, &path_proof], writer)?;
    let mut prepared = origins::record(projection.input, path_proof.clone(), writer)?;
    records::field(
        &mut prepared,
        "/data/payload/state",
        selector.clone(),
        writer,
    )?;
    records::subtree(&mut prepared, "/data/payload/name", name_proof, writer)?;
    records::subtree(&mut prepared, "/data/payload/path", path_proof, writer)?;
    let preceding_filepath = filepath(list, directive, projection, &mut prepared, writer)?;
    let rule = Rule::parse(name.value, path.value, preceding_filepath.matching_scope());
    let selection = expansion_selection(&rule, selector, list, directive, projection, writer)?;
    prepared.write(
        writer,
        &Subresource::new(wire::WILDCARD, indices.to_vec()),
        Status::Complete,
        &DefinitionPayload::Wildcard(Wildcard {
            state: WildcardState::Unexpanded,
            name,
            path,
            preceding_filepath,
        }),
        &[],
    )?;
    wildcard_expansion::write(projection, rule, indices, &selection, writer)?;
    Ok(())
}

fn expansion_selection(
    rule: &Result<Rule<'_>, Unexpanded>,
    selector: OriginContributions,
    list: SourceElement<'_>,
    directive: Option<&Directive<'_>>,
    projection: &DefinitionProjection<'_, '_>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, Failure> {
    let scoped = match rule {
        Ok(rule) => rule.uses_scope(),
        Err(Unexpanded::UnsupportedPattern) => false,
        Err(_) => true,
    };
    if !scoped {
        return Ok(selector);
    }
    let scope = if let Some(directive) = directive {
        directive.evidence(projection, writer)?.selection
    } else {
        origins::position(
            projection.input,
            element_position(&list.position())?,
            SourceUseRole::Selector,
            writer,
        )?
    };
    combine([&selector, &scope], writer)
}

fn filepath<'source>(
    list: SourceElement<'source>,
    directive: Option<&Directive<'source>>,
    projection: &DefinitionProjection<'_, '_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<FilepathScope<'source>, Failure> {
    let Some(directive) = directive else {
        let list_position = element_position(&list.position())?;
        records::subtree(
            prepared,
            "/data/payload/preceding_filepath",
            origins::position(
                projection.input,
                list_position,
                SourceUseRole::Selector,
                writer,
            )?,
            writer,
        )?;
        return Ok(FilepathScope::Missing { list_position });
    };
    let directive_position = element_position(&directive.element.position())?;
    let element = origins::position(
        projection.input,
        directive_position,
        SourceUseRole::Selector,
        writer,
    )?;
    let evidence = directive.evidence(projection, writer)?;
    records::field(
        prepared,
        "/data/payload/preceding_filepath/state",
        combine([&element, &evidence.selection], writer)?,
        writer,
    )?;
    records::subtree(
        prepared,
        "/data/payload/preceding_filepath/directive_position",
        element,
        writer,
    )?;
    let Some(attributes) = &directive.attributes else {
        return Ok(FilepathScope::Malformed { directive_position });
    };
    let path = Attribute::from(&attributes.path)?;
    records::subtree(
        prepared,
        "/data/payload/preceding_filepath/path",
        evidence.original,
        writer,
    )?;
    Ok(if path.value.trim_end_matches(['/', '\\']).is_empty() {
        FilepathScope::Reset {
            directive_position,
            path,
        }
    } else {
        FilepathScope::Authored {
            directive_position,
            path,
        }
    })
}
