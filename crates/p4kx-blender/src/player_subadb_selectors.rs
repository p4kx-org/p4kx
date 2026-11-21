// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_adb_declarations::{AttributeSelection, Branch, BranchFragment};
use crate::soc_scene::SocSceneError;
use crate::soc_scene_origin::SocXmlPosition;

use super::{Kind, Lookup, Resolver, Selector, lookup, source};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AdbBranchSelectors {
    document: usize,
    index: usize,
    position: SourcedValue<SocXmlPosition>,
    global_tags: Selector,
    fragment_definitions: Vec<FragmentDefinition>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct FragmentDefinition {
    ordinal: usize,
    position: SourcedValue<SocXmlPosition>,
    name: AttributeSelection,
    scope: Option<usize>,
    lookup: Option<Lookup>,
}

impl Resolver<'_> {
    pub(super) fn branch(
        &mut self,
        document: usize,
        branch: &Branch,
    ) -> Result<AdbBranchSelectors, SocSceneError> {
        let global_tags = self.global(document, &branch.tags)?;
        let fragment_definitions = branch
            .fragments
            .iter()
            .map(|selection| self.branch_fragment(document, selection))
            .collect::<Result<_, _>>()?;
        Ok(AdbBranchSelectors {
            document,
            index: branch.ordinal,
            position: branch.position.clone(),
            global_tags,
            fragment_definitions,
        })
    }

    fn branch_fragment(
        &mut self,
        document: usize,
        fragment: &BranchFragment,
    ) -> Result<FragmentDefinition, SocSceneError> {
        source::check(self.cancel)?;
        let resolution = if let [attribute] = fragment.name.occurrences.as_slice() {
            let name = attribute.value.value().ok_or(SocSceneError::Position)?;
            let index = self.root(document, Kind::Fragments)?;
            let scope = self.surveys.get(index).ok_or(SocSceneError::Position)?;
            Some((
                index,
                lookup(name, &fragment.name.origin, Some(scope), None),
            ))
        } else {
            None
        };
        let (scope, result) =
            resolution.map_or((None, None), |(index, result)| (Some(index), Some(result)));
        Ok(FragmentDefinition {
            ordinal: fragment.ordinal,
            position: fragment.position.clone(),
            name: fragment.name.clone(),
            scope,
            lookup: result,
        })
    }
}
