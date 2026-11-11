// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ptr;
use std::sync::Arc;

use crate::dataforge;
use crate::layout::Layouts;
use crate::read_observer::ReadObserver;
use crate::strings;
use crate::tree_owner::TreeOwner;
use crate::{
    CanonicalReadObserver, DataForge, DataForgeError, Guid, Location, Record, RecordMetadata,
    RecordTree, Step, StructValue,
};

use super::driver;
use super::materialization_budget::MaterializationBudget;
use super::positions::{Position, Positions};

/// An original weak slot waiting for the same canonical read to finish.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Pending {
    pub(crate) target: (u32, u32),
    pub(crate) at: Position,
}

/// Builds canonical Record trees and retains layout and position scratch between reads.
#[derive(Debug, Default)]
pub struct TreeBuilder<'source> {
    read_observer: Option<ReadObserver<'source>>,
    forge_id: usize,
    pub(super) materialization: MaterializationBudget,
    #[cfg(test)]
    pub(crate) peak_frames: usize,
    pub(crate) layouts: Layouts<'source>,
    pub(crate) owned: HashMap<(u32, u32), Position>,
    pub(crate) positions: Positions,
    pub(crate) weak: Vec<Pending>,
    pub(crate) locations: HashMap<(u32, u32), (&'source str, Arc<Location<'source>>)>,
    pub(crate) steps: Vec<Step<'source>>,
    pub(crate) position_scratch: Vec<u64>,
}

impl<'source> TreeBuilder<'source> {
    /// Creates a builder that observes each actual canonical read before returning it.
    #[must_use]
    pub fn with_read_observer(observer: Arc<dyn CanonicalReadObserver + 'source>) -> Self {
        let mut builder = Self::new();
        builder.set_read_observer(Some(observer));
        builder
    }

    /// Sets the observer without changing the cached layouts.
    pub fn set_read_observer(
        &mut self,
        observer: Option<Arc<dyn CanonicalReadObserver + 'source>>,
    ) {
        self.read_observer = observer.map(ReadObserver);
    }

    /// Creates a builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds the complete canonical tree without a materialization Value quota.
    ///
    /// # Errors
    ///
    /// Returns a source, addressability, allocation or construction error, or an observer stop.
    pub fn build(
        &mut self,
        record: &Record<'source>,
    ) -> Result<RecordTree<'source>, DataForgeError> {
        self.build_observed(record, None)
    }

    /// Builds one complete tree with a caller-owned limit on Field and Array-element Value slots.
    ///
    /// The implicit Record root, metadata and scratch storage are not counted or memory-bounded.
    ///
    /// # Errors
    ///
    /// Returns an explicit resource refusal without a partial tree, source/addressability errors,
    /// or an observer stop.
    pub fn build_with_budget(
        &mut self,
        record: &Record<'source>,
        maximum_values: u64,
    ) -> Result<RecordTree<'source>, DataForgeError> {
        self.build_observed(record, Some(maximum_values))
    }

    fn build_observed(
        &mut self,
        record: &Record<'source>,
        maximum_values: Option<u64>,
    ) -> Result<RecordTree<'source>, DataForgeError> {
        let outcome = self.build_tree(record, maximum_values);
        if let Some(observer) = &self.read_observer
            && observer.0.observe(record, outcome.as_ref()).is_break()
        {
            return Err(DataForgeError::ObservationStopped);
        }
        outcome
    }

    fn build_tree(
        &mut self,
        record: &Record<'source>,
        maximum_values: Option<u64>,
    ) -> Result<RecordTree<'source>, DataForgeError> {
        let forge = record.forge;
        self.start(forge, maximum_values);
        let def = record.def;
        let instance = u32::from(def.instance_index);
        let layout = self.layouts.get(forge, def.struct_index)?;
        let metadata = RecordMetadata::new(
            record.stored_file_name()?,
            record.stored_name()?,
            record.stored_tag()?,
        );
        let bytes = forge.instance(def.struct_index, instance).ok_or_else(|| {
            dataforge::out_of_range("Record", record.index as usize, "instance", instance.into())
        })?;
        self.owned
            .insert((def.struct_index, instance), Position::ROOT);
        let type_name = layout.type_name;
        let fields = driver::fields(self, forge, def.struct_index, layout, bytes)?;
        let mut tree = RecordTree {
            record_index: record.index(),
            metadata,
            guid: Guid::from_stored(def.guid),
            record_path: strings::strip_xml(metadata.file_name()),
            name: metadata.legacy_name(),
            tag: metadata.legacy_tag(),
            root: StructValue { type_name, fields },
            owner: TreeOwner::new(forge),
        };
        self.resolve_weak(forge, &mut tree)?;
        Ok(tree)
    }

    fn start(&mut self, forge: &'source DataForge<'source>, maximum_values: Option<u64>) {
        self.materialization = MaterializationBudget::new(maximum_values);
        let id = ptr::from_ref(forge).addr();
        if self.forge_id != id {
            self.layouts.clear();
            self.forge_id = id;
        }
        #[cfg(test)]
        {
            self.peak_frames = 0;
        }
        self.owned.clear();
        self.positions.clear();
        self.weak.clear();
        self.locations.clear();
        self.position_scratch.clear();
    }
}
