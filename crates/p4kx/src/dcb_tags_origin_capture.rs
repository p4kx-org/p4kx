// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use p4kx_extraction::{
    DataForge, Guid, Record, RecordTree, SourceLocation, SourceScope, SourceUseRole, TagBuildEvent,
    TagPathEdgeKind, TagPathIncoming, TagRelationIssue, TagTree,
};

use super::read::{Canonical, HeaderField, header};
use super::{Counts, Node, Proof};

#[derive(Default)]
pub(super) struct Capture {
    canonical: BTreeMap<u32, Canonical>,
    failed_classification: Vec<u32>,
    readability: Proof,
    membership: Proof,
    topology: Proof,
    dangling: Proof,
    repeated: Proof,
    skipped: BTreeMap<u32, Proof>,
    chosen: BTreeMap<Guid, usize>,
    nodes: Vec<Node>,
}

struct Choice<'r, 'f> {
    record: &'r Record<'f>,
    position: usize,
    parent: Option<usize>,
    incoming: Option<TagPathIncoming<'r, 'f>>,
    listed: bool,
}

impl Capture {
    pub(super) fn event(
        &mut self,
        scope: &SourceScope,
        forge: &DataForge<'_>,
        event: &TagBuildEvent<'_, '_>,
    ) -> Result<()> {
        match event {
            TagBuildEvent::MetadataFailed { record, .. } => {
                check_owner(forge, record)?;
                header(
                    scope,
                    record.index(),
                    HeaderField::SourceType,
                    SourceUseRole::Attempted,
                )?;
                self.failed_classification.push(record.index());
            }
            TagBuildEvent::CanonicalRead { record, outcome } => {
                check_owner(forge, record)?;
                self.canonical(scope, record, (*outcome).ok())?;
            }
            TagBuildEvent::DuplicateSkipped { record } => {
                check_owner(forge, record)?;
                ensure!(
                    self.canonical.remove(&record.index()).is_some(),
                    "missing original duplicate Tag"
                );
            }
            TagBuildEvent::Projected { record } => {
                check_owner(forge, record)?;
                let array = self.read(record.index())?.array(scope)?;
                self.topology.include(&array);
            }
            TagBuildEvent::PathChosen {
                record,
                position,
                parent_position,
                incoming,
                listed,
                ..
            } => {
                check_owner(forge, record)?;
                self.choose(
                    scope,
                    &Choice {
                        record,
                        position: usize::try_from(*position)?,
                        parent: parent_position.map(usize::try_from).transpose()?,
                        incoming: *incoming,
                        listed: *listed,
                    },
                )?;
            }
            TagBuildEvent::RelationSkipped {
                incoming,
                target,
                reason,
            } => {
                check_owner(forge, incoming.record())?;
                self.skip(scope, *incoming, *target, *reason)?;
            }
            TagBuildEvent::Cancelled => anyhow::bail!("Tag provenance capture cancelled"),
        }
        Ok(())
    }

    fn read(&self, index: u32) -> Result<&Canonical> {
        self.canonical
            .get(&index)
            .ok_or_else(|| anyhow::anyhow!("missing original Tag Record {index}"))
    }

    fn canonical(
        &mut self,
        scope: &SourceScope,
        record: &Record<'_>,
        tree: Option<&RecordTree<'_>>,
    ) -> Result<()> {
        let index = record.index();
        self.readability.uses.push(header(
            scope,
            index,
            HeaderField::SourceType,
            SourceUseRole::Selector,
        )?);
        let Some(tree) = tree else {
            scope.at(
                SourceLocation::Subresource {
                    namespace: "dataforgeRecordBody".to_owned(),
                    indices: vec![u64::from(index)],
                    field: "body".to_owned(),
                },
                SourceUseRole::Attempted,
            )?;
            self.readability.complete = false;
            return Ok(());
        };
        let read = Canonical::read(record, tree)?;
        if read.relations == "children" {
            self.membership.uses.push(header(
                scope,
                index,
                HeaderField::Guid,
                SourceUseRole::Selector,
            )?);
        }
        ensure!(
            self.canonical.insert(index, read).is_none(),
            "duplicate canonical Tag read"
        );
        Ok(())
    }

    fn choose(&mut self, scope: &SourceScope, choice: &Choice<'_, '_>) -> Result<()> {
        let index = choice.record.index();
        let read = self.read(index)?;
        ensure!(
            choice.position == self.nodes.len() && read.relations == "children",
            "invalid Tag output position"
        );
        let mut qualification = Proof::one(
            header(scope, index, HeaderField::Guid, SourceUseRole::Selector)?,
            true,
        );
        qualification.uses.push(header(
            scope,
            index,
            HeaderField::SourceType,
            SourceUseRole::Selector,
        )?);
        let incoming = choice
            .incoming
            .map(|incoming| self.chosen_edge(scope, choice, incoming))
            .transpose()?;
        let node = Node {
            index,
            guid: choice.record.guid(),
            parent: choice.parent,
            listed: choice.listed,
            qualification,
            incoming,
            guid_payload: header(scope, index, HeaderField::Guid, SourceUseRole::Payload)?,
            name: read.name(scope, SourceUseRole::Payload)?,
            path_name: read.name(scope, SourceUseRole::Conversion)?,
            legacy: read.legacy(scope)?,
            children: read.array(scope)?,
        };
        if let Some(incoming) = &node.incoming {
            self.topology.include(incoming);
        }
        ensure!(
            self.chosen.insert(node.guid, choice.position).is_none(),
            "duplicate selected Tag GUID"
        );
        self.nodes.push(node);
        Ok(())
    }

    fn chosen_edge(
        &self,
        scope: &SourceScope,
        choice: &Choice<'_, '_>,
        incoming: TagPathIncoming<'_, '_>,
    ) -> Result<Proof> {
        let owner = incoming.record().index();
        match (choice.parent, incoming.kind()) {
            (None, TagPathEdgeKind::Root) => ensure!(choice.listed, "unlisted Tag root edge"),
            (Some(parent), TagPathEdgeKind::Child) => {
                let parent = self
                    .nodes
                    .get(parent)
                    .ok_or_else(|| anyhow::anyhow!("missing selected Tag parent"))?;
                ensure!(
                    parent.index == owner && parent.listed == choice.listed,
                    "foreign selected Tag parent"
                );
            }
            _ => anyhow::bail!("invalid selected Tag relation"),
        }
        let mut edge = self
            .read(owner)?
            .edge(scope, incoming, Some(choice.record.guid()))?;
        if choice.parent.is_none() {
            edge.uses.push(header(
                scope,
                owner,
                HeaderField::SourceType,
                SourceUseRole::Selector,
            )?);
        }
        Ok(edge)
    }

    fn skip(
        &mut self,
        scope: &SourceScope,
        incoming: TagPathIncoming<'_, '_>,
        target: Option<Guid>,
        reason: TagRelationIssue,
    ) -> Result<()> {
        let index = incoming.record().index();
        let mut proof = self.read(index)?.edge(scope, incoming, target)?;
        match reason {
            TagRelationIssue::Dangling | TagRelationIssue::MalformedValue => {
                self.dangling.include(&proof);
            }
            TagRelationIssue::Repeated => {
                let winner = target
                    .and_then(|guid| self.chosen.get(&guid))
                    .and_then(|position| self.nodes.get(*position))
                    .ok_or_else(|| anyhow::anyhow!("missing first-selected Tag relation"))?;
                proof.include(&winner.qualification);
                if let Some(edge) = &winner.incoming {
                    proof.include(edge);
                } else {
                    proof.complete = false;
                }
                self.repeated.include(&proof);
            }
        }
        self.skipped.entry(index).or_default().include(&proof);
        Ok(())
    }

    pub(super) fn finish(
        mut self,
        scope: &SourceScope,
        record_count: usize,
        tree: &TagTree,
    ) -> Result<(Vec<Node>, Counts)> {
        ensure!(
            self.nodes.len() == tree.len(),
            "incomplete Tag origin trace"
        );
        for (tag, node) in tree.iter().zip(&mut self.nodes) {
            ensure!(
                tag.guid() == node.guid
                    && tag.parent().map(|parent| parent.position()) == node.parent
                    && tag.is_listed() == node.listed
                    && node.incoming.is_some() == (node.parent.is_some() || node.listed),
                "Tag origin trace differs from selected tree"
            );
            if let Some(skipped) = self.skipped.remove(&node.index) {
                node.children.include(&skipped);
            }
        }
        let classification = classification(scope, record_count, &self.failed_classification)?;
        self.readability.include(&classification);
        self.membership.include(&self.readability);
        self.topology.include(&self.membership);
        Ok((
            self.nodes,
            Counts {
                classification,
                readability: self.readability,
                membership: self.membership,
                topology: self.topology,
                dangling: self.dangling,
                repeated: self.repeated,
            },
        ))
    }
}

fn check_owner(forge: &DataForge<'_>, record: &Record<'_>) -> Result<()> {
    ensure!(record.is_from(forge), "foreign Tag Record");
    Ok(())
}

fn classification(scope: &SourceScope, record_count: usize, failures: &[u32]) -> Result<Proof> {
    let count = u64::try_from(record_count)?;
    let mut proof = Proof::one(
        scope.at(
            SourceLocation::Subresource {
                namespace: "dataforgeHeader".to_owned(),
                indices: Vec::new(),
                field: "recordCount".to_owned(),
            },
            SourceUseRole::Selector,
        )?,
        failures.is_empty(),
    );
    let mut start = 0;
    for end in failures.iter().copied().map(u64::from).chain([count]) {
        ensure!(
            start <= end && end <= count,
            "invalid Tag classification survey"
        );
        if start < end {
            // The bounds address the original successfully classified Record interval.
            proof.uses.push(scope.at(
                SourceLocation::Subresource {
                    namespace: "dataforgeRecordHeaderRange".to_owned(),
                    indices: vec![start, end],
                    field: "sourceType".to_owned(),
                },
                SourceUseRole::Selector,
            )?);
        }
        start = end + 1;
    }
    Ok(proof)
}
