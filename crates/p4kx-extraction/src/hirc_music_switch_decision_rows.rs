// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_audio::{
    BankMusicSwitchBranch, BankMusicSwitchFields, BankMusicSwitchGroup, BankMusicSwitchNode,
    BankMusicSwitchRow, BankMusicSwitchTarget,
};
use schemars::JsonSchema;
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::hirc_view::StateRange;
use crate::source_evidence_error::SourceEvidenceError;
use crate::source_use::SourceUseRole;
use crate::source_value::SourcedValue;

use super::super::super::origins::ObjectOrigins;
use super::Owner;

const CHILD_LINK: &str = "musicSwitchChildLink";
const TARGET_ID: &str = "musicSwitchTargetId";

pub(in crate::hirc_view) struct Rows<'view, 'input, R> {
    owner: &'view RefCell<Owner<'input, R>>,
    origins: &'view ObjectOrigins,
    selectors: Selectors,
}

impl<'view, 'input, R> Rows<'view, 'input, R> {
    pub(super) fn new(
        owner: &'view RefCell<Owner<'input, R>>,
        origins: &'view ObjectOrigins,
        fields: &BankMusicSwitchFields,
    ) -> Self {
        Self {
            owner,
            origins,
            selectors: Selectors {
                group_count: fields.group_count.as_ref().map(|word| word.range.clone()),
                tree_size: fields.tree_size.as_ref().map(|word| word.range.clone()),
                tree: fields.tree_range.clone(),
            },
        }
    }
}

impl<R: Read + Seek> Serialize for Rows<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut owner = self.owner.try_borrow_mut().map_err(S::Error::custom)?;
        let Owner::Switch(node) = &mut *owner else {
            return Err(S::Error::custom("expected the original MusicSwitch"));
        };
        let mut rows = node.decision_rows();
        let mut output = serializer.serialize_seq(None)?;
        while let Some(row) = rows.next() {
            match row.map_err(S::Error::custom)? {
                BankMusicSwitchRow::Group { ordinal, value } => {
                    let group = DecisionGroup::read(self.origins, ordinal, &value)
                        .map_err(S::Error::custom)?;
                    output.serialize_element(&DecisionRow::Group(&group))?;
                }
                BankMusicSwitchRow::Node { ordinal, value } => {
                    let node = DecisionNode::read(self.origins, ordinal, &value)
                        .map_err(S::Error::custom)?;
                    output.serialize_element(&DecisionRow::Node(&node))?;
                }
                BankMusicSwitchRow::Target { ordinal, value } => {
                    let target = self
                        .selectors
                        .target(self.origins, ordinal, &value, rows.current_target_route())
                        .map_err(S::Error::custom)?;
                    output.serialize_element(&DecisionRow::Target(&target))?;
                }
                _ => {
                    return Err(S::Error::custom(
                        "unexpected original MusicSwitch decision row",
                    ));
                }
            }
        }
        output.end()
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(in crate::hirc_view) enum DecisionRow<'row> {
    Group(&'row DecisionGroup),
    Node(&'row DecisionNode),
    Target(&'row DecisionTarget),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct DecisionGroup {
    ordinal: SourcedValue<u32>,
    group_id: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_code: Option<SourcedValue<u8>>,
}

impl DecisionGroup {
    fn read(
        origins: &ObjectOrigins,
        ordinal: u32,
        group: &BankMusicSwitchGroup,
    ) -> Result<Self, SourceEvidenceError> {
        Ok(Self {
            ordinal: origins.derived(ordinal, &group.id.range, "musicSwitchGroupPosition")?,
            group_id: origins.field(&group.id, "musicSwitchGroupId")?,
            type_code: origins.optional(group.type_code.as_ref(), "musicSwitchGroupType")?,
        })
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct DecisionNode {
    ordinal: SourcedValue<u32>,
    key: SourcedValue<u32>,
    link_word: SourcedValue<u32>,
    weight: SourcedValue<u16>,
    probability: SourcedValue<u16>,
    node_range: SourcedValue<StateRange>,
}

impl DecisionNode {
    fn read(
        origins: &ObjectOrigins,
        ordinal: u32,
        node: &BankMusicSwitchNode,
    ) -> Result<Self, SourceEvidenceError> {
        Ok(Self {
            ordinal: origins.derived(ordinal, &node.range, "musicSwitchNodePosition")?,
            key: origins.field(&node.key, "musicSwitchNodeKey")?,
            link_word: origins.field(&node.link, "musicSwitchNodeLink")?,
            weight: origins.field(&node.weight, "musicSwitchNodeWeight")?,
            probability: origins.field(&node.probability, "musicSwitchNodeProbability")?,
            node_range: origins.range(&node.range, "musicSwitchNodeExtent")?,
        })
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct DecisionTarget {
    ordinal: SourcedValue<u32>,
    node_index: SourcedValue<u32>,
    level: SourcedValue<u32>,
    target_id: SourcedValue<u32>,
    ancestor_nodes: Vec<SourcedValue<u32>>,
}

struct Selectors {
    group_count: Option<Range<u64>>,
    tree_size: Option<Range<u64>>,
    tree: Option<Range<u64>>,
}

impl Selectors {
    fn target<'path>(
        &self,
        origins: &ObjectOrigins,
        ordinal: u32,
        target: &BankMusicSwitchTarget,
        path: impl Iterator<Item = &'path BankMusicSwitchBranch>,
    ) -> Result<DecisionTarget, SourceEvidenceError> {
        use SourceUseRole::{Conversion, Payload, Selector};

        let mut evidence = Vec::new();
        for (label, range) in [
            ("musicSwitchGroupCount", self.group_count.as_ref()),
            ("musicSwitchTreeSize", self.tree_size.as_ref()),
            ("musicSwitchTreeExtent", self.tree.as_ref()),
        ] {
            if let Some(range) = range {
                evidence.push(origins.reference(label, range, Selector)?);
            }
        }
        let mut ancestor_nodes = Vec::new();
        for branch in path {
            let link = origins.reference(CHILD_LINK, &branch.link.range, Selector)?;
            let position =
                origins.reference("musicSwitchBranchPosition", &branch.link.range, Conversion)?;
            ancestor_nodes.push(origins.value(branch.node_index, &[link.clone(), position]));
            evidence.push(link);
        }
        evidence.push(origins.reference(TARGET_ID, &target.id.range, Selector)?);
        let ordinal = origins.value(ordinal, &evidence);
        let node_index = origins.value(target.node_index, &evidence);
        let level = origins.value(target.level, &evidence);
        evidence.push(origins.reference(TARGET_ID, &target.id.range, Payload)?);
        Ok(DecisionTarget {
            ordinal,
            node_index,
            level,
            target_id: origins.value(target.id.value, &evidence),
            ancestor_nodes,
        })
    }
}
