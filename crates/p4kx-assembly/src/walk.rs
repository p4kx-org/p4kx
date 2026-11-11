// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::LoadoutTargetGuid;
use p4kx_dataforge::Guid;

use crate::plan::Plan;
use crate::{LoadoutError, MAX_LOADOUT_DEPTH, MAX_LOADOUT_PORTS};

/// The state of one resolution: the Entities on the current branch, the ports so far and the
/// changes being applied.
pub(crate) struct Walk {
    pub(crate) ancestors: Vec<Guid>,
    pub(crate) nesting: usize,
    ports: usize,
    pub(crate) plan: Option<Plan>,
    pub(crate) cancellable: bool,
}

impl Walk {
    pub(crate) fn new(root: Guid, plan: Option<Plan>) -> Self {
        Self {
            ancestors: vec![root],
            nesting: 0,
            ports: 0,
            plan,
            cancellable: false,
        }
    }

    /// Steps into the port `name` for the changes.
    pub(crate) fn enter(&mut self, name: &str) {
        if let Some(plan) = self.plan.as_mut() {
            plan.enter(name);
        }
    }

    /// Steps out of the port entered last.
    pub(crate) fn leave(&mut self) {
        if let Some(plan) = self.plan.as_mut() {
            plan.leave();
        }
    }

    /// Tells the changes that the Entity at the port entered last is not expanded.
    pub(crate) fn stopped_here(&mut self) {
        if let Some(plan) = self.plan.as_mut() {
            plan.stopped_here();
        }
    }

    /// Returns whether the Entity being built is the root of the tree.
    pub(crate) const fn is_root(&self) -> bool {
        self.ancestors.len() == 1
    }

    /// Returns the depth of the current branch: attached Entities plus nested Loadout levels.
    pub(crate) const fn depth(&self) -> usize {
        self.ancestors.len().saturating_add(self.nesting)
    }

    /// Counts one more port, or returns `false` when the tree already holds
    /// [`MAX_LOADOUT_PORTS`].
    pub(crate) const fn reserve(&mut self) -> bool {
        if self.ports >= MAX_LOADOUT_PORTS {
            return false;
        }
        self.ports = self.ports.saturating_add(1);
        true
    }

    /// Returns why the Entity `guid` may not be attached below the current branch, if it may not.
    pub(crate) fn limit(&self, guid: Guid) -> Option<LoadoutError> {
        if self.ancestors.contains(&guid) {
            Some(LoadoutError::Cycle {
                guid: LoadoutTargetGuid::unbound(guid),
            })
        } else if self.depth() > MAX_LOADOUT_DEPTH {
            Some(LoadoutError::TooDeep)
        } else {
            None
        }
    }
}
