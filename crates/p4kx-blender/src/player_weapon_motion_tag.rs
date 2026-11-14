// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{CancelToken, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_weapon_motion_sources::Weapon;
use crate::soc_scene::SocSceneError;

use super::tag_lookup::Status;
use super::{AdbTagSelectors, TagScope, source};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WeaponMotionTag {
    scope: SourcedValue<usize>,
    basis: SourcedValue<MatchBasis>,
    weapon_tokens: Vec<WeaponToken>,
    option_tokens: Vec<OptionToken>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
enum MatchBasis {
    Unique,
    Competing,
    Incomplete,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct WeaponToken {
    weapon: SourcedValue<usize>,
    ordinal: SourcedValue<usize>,
    spelling: SourcedValue<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct OptionToken {
    document: SourcedValue<usize>,
    option: SourcedValue<usize>,
    ordinal: SourcedValue<usize>,
    spelling: SourcedValue<String>,
}

pub(super) fn compare(
    weapons: &[Weapon],
    options: &[AdbTagSelectors],
    scopes: &[TagScope],
    sources: &SourceUses,
    cancel: &CancelToken,
) -> Result<Vec<WeaponMotionTag>, SocSceneError> {
    let survey = TagSurvey {
        scopes,
        sources,
        cancel,
    };
    let by_name = weapon_tokens(weapons, cancel)?;
    let mut groups = BTreeMap::new();
    for option in options {
        source::check(cancel)?;
        survey.option(option, &by_name, &mut groups)?;
    }
    Ok(groups.into_values().collect())
}

struct TagSurvey<'a> {
    scopes: &'a [TagScope],
    sources: &'a SourceUses,
    cancel: &'a CancelToken,
}

impl TagSurvey<'_> {
    fn option(
        &self,
        option: &AdbTagSelectors,
        by_name: &BTreeMap<String, Vec<WeaponToken>>,
        groups: &mut BTreeMap<(usize, String), WeaponMotionTag>,
    ) -> Result<(), SocSceneError> {
        let Some(scope) = option.global.root_scope else {
            return Ok(());
        };
        let original = self.scopes.get(scope).ok_or(SocSceneError::Position)?;
        let [attribute] = option.global.attribute.occurrences.as_slice() else {
            return Err(SocSceneError::Position);
        };
        for (ordinal, token) in option.global.tokens.iter().enumerate() {
            source::check(self.cancel)?;
            let Some(basis) = basis(token.lookup.status, !token.lookup.matches.is_empty()) else {
                continue;
            };
            let spelling = token.spelling.to_ascii_lowercase();
            let Some(weapon_tokens) = by_name.get(&spelling) else {
                continue;
            };
            let group = match groups.entry((scope, spelling)) {
                std::collections::btree_map::Entry::Occupied(group) => group.into_mut(),
                std::collections::btree_map::Entry::Vacant(group) => {
                    group.insert(WeaponMotionTag {
                        scope: self
                            .sources
                            .value_with_origins(SourceValue::Present(scope), &[&original.origin])?,
                        basis: self
                            .sources
                            .value_with_origins(SourceValue::Present(basis), &[&original.origin])?,
                        weapon_tokens: weapon_tokens.clone(),
                        option_tokens: Vec::new(),
                    })
                }
            };
            if group.basis.value() != Some(&basis) {
                return Err(SocSceneError::Position);
            }
            group.option_tokens.push(OptionToken {
                document: attribute.value.map_ref(|_| option.document),
                option: attribute.value.map_ref(|_| option.index),
                ordinal: attribute.value.map_ref(|_| ordinal),
                spelling: attribute.value.map_ref(|_| token.spelling.clone()),
            });
        }
        Ok(())
    }
}

fn weapon_tokens(
    weapons: &[Weapon],
    cancel: &CancelToken,
) -> Result<BTreeMap<String, Vec<WeaponToken>>, SocSceneError> {
    let mut by_name = BTreeMap::new();
    for (weapon, row) in weapons.iter().enumerate() {
        source::check(cancel)?;
        let Some(raw) = row.tags.value() else {
            continue;
        };
        for (ordinal, spelling) in raw.split_whitespace().enumerate() {
            source::check(cancel)?;
            let target: &mut Vec<_> = by_name.entry(spelling.to_ascii_lowercase()).or_default();
            target.push(WeaponToken {
                weapon: row.component.map_ref(|_| weapon),
                ordinal: row.tags.map_ref(|_| ordinal),
                spelling: row.tags.map_ref(|_| spelling.to_owned()),
            });
        }
    }
    Ok(by_name)
}

const fn basis(status: Status, observed_match: bool) -> Option<MatchBasis> {
    if !observed_match {
        return None;
    }
    match status {
        Status::Resolved => Some(MatchBasis::Unique),
        Status::Ambiguous => Some(MatchBasis::Competing),
        Status::Incomplete => Some(MatchBasis::Incomplete),
        _ => None,
    }
}
