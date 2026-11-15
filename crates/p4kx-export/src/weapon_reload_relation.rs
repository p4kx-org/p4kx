// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::original::PhysicalEntryRef;
use crate::catalog_v3::catalog_item_components::item_definition::WeaponAttachment;
use crate::schema_fields::every_field_required;

use super::document::OptionRow;
use super::graph::{BranchLink, attribute};
use super::scope::{DatabaseScopes, Declaration};

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum FragmentStatus {
    UnresolvedTarget,
    UnreadableSelector,
    NoMatchingGroup,
    IncompleteScope,
    DifferentScope,
    Matched,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct FragmentMatch {
    source: PhysicalEntryRef,
    branch: SourcedValue<SourceLocation>,
    selector: SourcedValue<String>,
    status: SourcedValue<FragmentStatus>,
    options: Vec<SourcedValue<SourceLocation>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct WeaponPlayerAnimationCandidate {
    weapon_record_index: SourcedValue<u32>,
    attachment_component_ordinal: SourcedValue<u32>,
    player_record_index: SourcedValue<u32>,
    player_component_ordinal: SourcedValue<u32>,
    branch_database: PhysicalEntryRef,
    branch_position: SourcedValue<SourceLocation>,
    option_database: PhysicalEntryRef,
    option_position: SourcedValue<SourceLocation>,
    matches: Vec<TagMatch>,
    tag_evidence: SourcedValue<TagEvidence>,
    playback: SourcedValue<Playback>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct TagMatch {
    weapon_token: SourcedValue<usize>,
    option_token: SourcedValue<usize>,
    source: PhysicalEntryRef,
    declaration: SourcedValue<SourceLocation>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum TagEvidence {
    SharedDeclaration,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum Playback {
    Unverified,
}

pub(super) fn fragments(
    edge: &BranchLink,
    source_scope: Option<&DatabaseScopes>,
    target_scope: Option<&DatabaseScopes>,
    options: Option<&[OptionRow]>,
    sources: &SourceUses,
) -> Result<Vec<FragmentMatch>, CatalogError> {
    edge.fragment_selectors
        .iter()
        .map(|selector| {
            let group = matching_options(selector, options);
            let (status, definitions) =
                fragment_status(selector, source_scope, target_scope, options, &group);
            let mut origins = vec![selector.origin(), edge.target.origin()];
            origins.extend(
                definitions
                    .iter()
                    .map(|definition| definition.name.origin()),
            );
            origins.extend(group.first().map(|option| option.fragment.origin()));
            origins.extend(source_scope.map(|scope| scope.fragments.coverage().origin()));
            origins.extend(target_scope.map(|scope| scope.fragments.coverage().origin()));
            Ok(FragmentMatch {
                source: edge.source.clone(),
                branch: edge.position.clone(),
                selector: selector.clone(),
                status: sources.value_with_origins(SourceValue::Present(status), &origins)?,
                options: group.iter().map(|option| option.position.clone()).collect(),
            })
        })
        .collect()
}

fn matching_options<'a>(
    selector: &SourcedValue<String>,
    options: Option<&'a [OptionRow]>,
) -> Vec<&'a OptionRow> {
    let Some(name) = selector
        .value()
        .filter(|value| !value.is_empty() && value.is_ascii())
    else {
        return Vec::new();
    };
    options
        .into_iter()
        .flatten()
        .filter(|option| {
            option
                .fragment
                .value()
                .is_some_and(|label| label.eq_ignore_ascii_case(name))
        })
        .collect()
}

fn fragment_status<'a>(
    selector: &SourcedValue<String>,
    source_scope: Option<&'a DatabaseScopes>,
    target_scope: Option<&'a DatabaseScopes>,
    options: Option<&[OptionRow]>,
    group: &[&OptionRow],
) -> (FragmentStatus, Vec<&'a Declaration>) {
    let Some(name) = selector
        .value()
        .filter(|value| !value.is_empty() && value.is_ascii())
    else {
        return (FragmentStatus::UnreadableSelector, Vec::new());
    };
    if options.is_none() {
        return (FragmentStatus::UnresolvedTarget, Vec::new());
    }
    if group.is_empty() {
        return (FragmentStatus::NoMatchingGroup, Vec::new());
    }
    let (Some(source), Some(target)) = (source_scope, target_scope) else {
        return (FragmentStatus::IncompleteScope, Vec::new());
    };
    let source_names: Vec<_> = source.fragments.named(name).collect();
    let target_names: Vec<_> = target.fragments.named(name).collect();
    let status = match (source_names.as_slice(), target_names.as_slice()) {
        ([left], [right])
            if source.fragments.coverage().value() == Some(&true)
                && target.fragments.coverage().value() == Some(&true) =>
        {
            if left.source == right.source && left.position.value() == right.position.value() {
                FragmentStatus::Matched
            } else {
                FragmentStatus::DifferentScope
            }
        }
        _ => FragmentStatus::IncompleteScope,
    };
    (
        status,
        source_names.into_iter().chain(target_names).collect(),
    )
}

pub(super) struct CandidateInput<'a, 'source> {
    pub(super) weapon: &'a WeaponAttachment<'a, 'source>,
    pub(super) actor: &'a SourcedValue<u32>,
    pub(super) component: &'a SourcedValue<u32>,
    pub(super) edge: &'a BranchLink,
    pub(super) target_id: &'a PhysicalEntryRef,
    pub(super) target_scope: &'a DatabaseScopes,
    pub(super) option: &'a OptionRow,
    pub(super) fragment_results: &'a [FragmentMatch],
}

impl CandidateInput<'_, '_> {
    pub(super) fn project(
        self,
        sources: &SourceUses,
    ) -> Result<Option<WeaponPlayerAnimationCandidate>, CatalogError> {
        let fragments = self
            .fragment_results
            .iter()
            .filter(|row| {
                matches!(row.status.value(), Some(FragmentStatus::Matched))
                    && row
                        .options
                        .iter()
                        .any(|option| option.value() == self.option.position.value())
            })
            .collect::<Vec<_>>();
        if fragments.is_empty() {
            return Ok(None);
        }
        let tags = attribute(
            &self.option.attributes,
            "Tags",
            &self.option.position,
            sources,
        )?;
        let evidence = self.match_tags(&tags);
        if evidence.is_empty() {
            return Ok(None);
        }
        let mut inputs = vec![
            self.weapon.item_type.origin(),
            self.weapon.tags.origin(),
            self.actor.origin(),
            self.component.origin(),
            self.edge.tags.origin(),
            self.edge.target.origin(),
            tags.origin(),
            self.target_scope.tags.coverage().origin(),
        ];
        inputs.extend(fragments.iter().map(|fragment| fragment.status.origin()));
        inputs.extend(evidence.iter().map(|tag| tag.declaration.origin()));
        let tag_evidence = sources.value_with_origins(
            SourceValue::Present(TagEvidence::SharedDeclaration),
            &inputs,
        )?;
        let playback = tag_evidence.map_ref(|_| Playback::Unverified);
        Ok(Some(WeaponPlayerAnimationCandidate {
            weapon_record_index: self.weapon.record.clone(),
            attachment_component_ordinal: self.weapon.slot.clone(),
            player_record_index: self.actor.clone(),
            player_component_ordinal: self.component.clone(),
            branch_database: self.edge.source.clone(),
            branch_position: self.edge.position.clone(),
            option_database: self.target_id.clone(),
            option_position: self.option.position.clone(),
            matches: evidence,
            tag_evidence,
            playback,
        }))
    }

    fn match_tags(&self, tags: &SourcedValue<String>) -> Vec<TagMatch> {
        let mut output = Vec::new();
        let Some(weapon_names) = self.weapon.tags.value() else {
            return output;
        };
        let Some(option_names) = tags.value() else {
            return output;
        };
        if self.target_scope.tags.coverage().value() != Some(&true) {
            return output;
        }
        for (option_index, option) in option_names.split('+').enumerate() {
            let option_name = option.trim();
            if option_name.is_empty() || !option_name.is_ascii() {
                continue;
            }
            let candidates: Vec<_> = self.target_scope.tags.named(option_name).collect();
            let [declaration] = candidates.as_slice() else {
                continue;
            };
            for (weapon_index, weapon_name) in weapon_names.split_whitespace().enumerate() {
                if weapon_name.eq_ignore_ascii_case(option_name) {
                    output.push(TagMatch {
                        weapon_token: self.weapon.tags.map_ref(|_| weapon_index),
                        option_token: tags.map_ref(|_| option_index),
                        source: declaration.source.clone(),
                        declaration: declaration.position.clone(),
                    });
                }
            }
        }
        output
    }
}
