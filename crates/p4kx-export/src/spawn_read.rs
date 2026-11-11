// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, RecordTree, StructValue, TagTree};

use crate::entity_spawn_model::EntitySpawnPool;
use crate::npc_spawn_model::NpcSpawnPool;
use crate::record_read::ReadError;
use crate::source_fields::FieldCapture;
use crate::spawn_model::{LocationQuery, ShipSpawnPool};

#[path = "spawn_content_read.rs"]
mod content;
#[path = "spawn_read_input.rs"]
mod input;
#[path = "spawn_location_read.rs"]
mod locations;
#[path = "spawn_reference_read.rs"]
mod references;
#[path = "spawn_ship_read.rs"]
mod ships;

pub(crate) use input::{ReadContext, ReadFields};

/// The normalized spawn content or query stated by one mission-property value.
#[derive(Debug)]
pub(crate) enum Spawn<'source> {
    Ships(ShipSpawnPool),
    Npcs(NpcSpawnPool<'source>),
    Entities(EntitySpawnPool),
    Location(LocationQuery),
}

/// The existing content algorithms and their optional actual-source access context.
pub(crate) struct Reader<'tree, 'source> {
    tags: &'tree TagTree,
    root: &'tree StructValue<'source>,
    owner: u32,
    owner_guid: Guid,
    context: Option<&'tree dyn ReadContext<'source>>,
    guard_capture: Option<&'tree dyn FieldCapture<'source>>,
}

impl<'tree, 'source> Reader<'tree, 'source> {
    /// Creates the interpreted reader of an existing canonical tree.
    pub(crate) const fn for_record(tags: &'tree TagTree, tree: &'tree RecordTree<'source>) -> Self {
        Self {
            tags,
            root: tree.root(),
            owner: tree.record_index(),
            owner_guid: tree.guid(),
            context: None,
            guard_capture: None,
        }
    }

    /// Binds existing guard failures to the admitted original tree.
    pub(crate) fn for_observed(
        tags: &'tree TagTree,
        tree: &'tree RecordTree<'source>,
        capture: &'tree dyn FieldCapture<'source>,
    ) -> Result<Self, ReadError> {
        capture.check_parent(tree.root())?;
        Ok(Self {
            guard_capture: Some(capture),
            ..Self::for_record(tags, tree)
        })
    }

    /// Binds actual content accesses to the checked original frame.
    pub(crate) fn for_context(
        tags: &'tree TagTree,
        tree: &'tree RecordTree<'source>,
        context: &'tree dyn ReadContext<'source>,
    ) -> Result<Self, ReadError> {
        context.check_tree(tree)?;
        Ok(Self {
            context: Some(context),
            ..Self::for_record(tags, tree)
        })
    }

    pub(crate) fn capture(&self) -> Option<&'tree dyn FieldCapture<'source>> {
        self.context
            .map(ReadContext::capture)
            .or(self.guard_capture)
    }

    pub(crate) const fn fields<'reader, 'parent>(
        &'reader self,
        parent: Option<&'parent StructValue<'source>>,
    ) -> ReadFields<'reader, 'parent, 'source> {
        ReadFields::new(self, parent)
    }
}

#[cfg(test)]
#[path = "spawn_observed_tests.rs"]
mod observed_tests;
#[cfg(test)]
#[path = "spawn_owner_tests.rs"]
mod owner_tests;
