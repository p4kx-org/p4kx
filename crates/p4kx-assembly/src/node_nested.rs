// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::plan::Action;
use crate::resolver::LoadoutResolver;
use crate::spec::{LoadoutSpec, TargetSpec};
use crate::walk::Walk;
use crate::{Attachment, LoadoutError, MAX_LOADOUT_DEPTH, PortNode, PortSource, PortVisibility};

impl LoadoutResolver<'_> {
    /// Applies the change of the plan to the port entered last, if it has one, and returns
    /// whether it holds what the plan says now.
    pub(crate) fn change(&mut self, node: &mut PortNode, walk: &mut Walk) -> bool {
        let Some(plan) = walk.plan.as_mut() else {
            return false;
        };
        match plan.action() {
            Action::Keep => false,
            Action::Exclude => {
                node.source = PortSource::Excluded;
                node.origin = None;
                true
            }
            Action::Replace(guid, facts) => {
                if walk.ancestors.contains(&guid) {
                    plan.reject_cycle(facts.class_name());
                    return false;
                }
                if let Some(constraints) = node.constraints()
                    && let Err(reason) = constraints.check(&facts)
                {
                    plan.mismatch(&facts, reason);
                }
                node.attachment = self.attach(&TargetSpec::Reference(guid), None, walk);
                node.source = PortSource::Replaced;
                node.origin = None;
                true
            }
        }
    }

    /// Resolves the entries of the Loadout of an empty port into ports of their own; whatever
    /// cannot be resolved or kept is reported in `issues`.
    pub(crate) fn nested_ports(
        &mut self,
        loadout: Option<&LoadoutSpec>,
        walk: &mut Walk,
        issues: &mut Vec<LoadoutError>,
    ) -> Vec<PortNode> {
        if self.observation_stopped() {
            return Vec::new();
        }
        let Some(loadout) = loadout else {
            return Vec::new();
        };
        if walk.depth() >= MAX_LOADOUT_DEPTH {
            issues.push(LoadoutError::TooDeep);
            return Vec::new();
        }
        let entries = match self.entries(Some(loadout)) {
            Ok(entries) => entries,
            Err(error) => {
                issues.push(error);
                return Vec::new();
            }
        };
        issues.extend(entries.issues.iter().cloned());
        let mut ports = Vec::new();
        walk.nesting = walk.nesting.saturating_add(1);
        for entry in entries.list.iter() {
            if self.observation_stopped() {
                break;
            }
            if !walk.reserve() {
                issues.push(LoadoutError::TooManyPorts);
                break;
            }
            walk.enter(&entry.port);
            let mut node = PortNode {
                name: entry.port.clone(),
                source: PortSource::Override,
                definition: None,
                constraints: None,
                attachment: Attachment::Empty,
                nested: Vec::new(),
                visibility: PortVisibility::default(),
                origin: Some(entry.origin.clone()),
            };
            if !self.change(&mut node, walk) {
                node.attachment = self.attach(&entry.target, entry.loadout.as_ref(), walk);
                if matches!(node.attachment, Attachment::Empty) {
                    node.nested = self.nested_ports(entry.loadout.as_ref(), walk, issues);
                }
            }
            walk.leave();
            ports.push(node);
        }
        walk.nesting = walk.nesting.saturating_sub(1);
        ports
    }
}
