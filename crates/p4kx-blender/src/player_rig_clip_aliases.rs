// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{CancelToken, FieldOrigin, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_adb_declarations::{AdbDeclarations, AnimationAlias};
use crate::player_source::{PlayerSourceError, check};
use crate::player_source_motion::RigOwnerState;

use super::Clip;

#[derive(Debug, Default)]
pub(crate) struct ClipAliases {
    pub(crate) groups: Vec<ClipAliasGroup>,
    pub(crate) occurrences: Vec<ClipAliasOccurrence>,
    pub(crate) candidates: Vec<ClipAliasCandidate>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClipAliasGroup {
    alias_key: SourcedValue<String>,
    controller_rig_binding: RigOwnerState,
    #[serde(skip)]
    selector: FieldOrigin,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClipAliasOccurrence {
    document: usize,
    fragment: usize,
    layer: SourcedValue<usize>,
    animation: SourcedValue<usize>,
    group: SourcedValue<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClipAliasCandidate {
    rig: usize,
    rule: usize,
    chunk: usize,
    record: usize,
    group: SourcedValue<usize>,
}

pub(super) fn collect(
    declarations: &[AdbDeclarations],
    clips: &[(usize, Clip)],
    sources: &SourceUses,
    cancel: &CancelToken,
) -> Result<ClipAliases, PlayerSourceError> {
    let mut aliases = ClipAliases::default();
    let mut groups = HashMap::new();
    for adb in declarations {
        check(cancel)?;
        for (fragment, authored) in adb.fragments.iter().enumerate() {
            check(cancel)?;
            for occurrence in authored.animation_aliases() {
                check(cancel)?;
                let group = aliases.group(sources, &mut groups, &occurrence)?;
                aliases.occurrences.push(ClipAliasOccurrence {
                    document: adb.document,
                    fragment,
                    layer: occurrence.layer,
                    animation: occurrence.animation,
                    group,
                });
            }
        }
    }
    for (rig, clip) in clips {
        check(cancel)?;
        let Some(index) = clip
            .alias_key
            .value()
            .and_then(|alias| groups.get(alias.as_str()))
            .copied()
        else {
            continue;
        };
        aliases.candidates.push(ClipAliasCandidate {
            rig: *rig,
            rule: clip.rule,
            chunk: clip.chunk,
            record: clip.record,
            group: aliases.membership(sources, &clip.alias_key, index)?,
        });
    }
    Ok(aliases)
}

impl ClipAliases {
    fn group<'names>(
        &mut self,
        sources: &SourceUses,
        index: &mut HashMap<&'names str, usize>,
        occurrence: &AnimationAlias<'names>,
    ) -> Result<SourcedValue<usize>, PlayerSourceError> {
        let Some(key) = &occurrence.key else {
            return Ok(occurrence.unavailable(sources)?);
        };
        let name = key.value.value().ok_or(PlayerSourceError::Binding)?;
        let index = *index.entry(name).or_insert_with(|| {
            let index = self.groups.len();
            self.groups.push(ClipAliasGroup {
                alias_key: key.value.clone(),
                controller_rig_binding: RigOwnerState::Unresolved,
                selector: sources.origin(std::slice::from_ref(key.selector), true),
            });
            index
        });
        self.membership(sources, key.value, index)
    }

    fn membership(
        &self,
        sources: &SourceUses,
        key: &SourcedValue<String>,
        index: usize,
    ) -> Result<SourcedValue<usize>, PlayerSourceError> {
        let group = self.groups.get(index).ok_or(PlayerSourceError::Binding)?;
        Ok(sources.value_with_origins(
            SourceValue::Present(index),
            &[key.origin(), &group.selector],
        )?)
    }
}
