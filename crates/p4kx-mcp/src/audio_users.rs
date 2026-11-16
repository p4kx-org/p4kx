// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_assembly::audio::{TriggerHit, TriggerOccurrence, TriggerUsers};
use p4kx_extraction::{DataForge, Record};

use crate::audio_shapes::EntitiesShape;
use crate::budget::Budget;
use crate::error::ToolError;

use super::header;

pub(super) struct Users<'forge> {
    forge: &'forge DataForge<'forge>,
    index: TriggerUsers<Option<TriggerOccurrence>>,
    listed: usize,
}

impl<'forge> Users<'forge> {
    pub(super) fn new(
        forge: &'forge DataForge<'forge>,
        hits: &[TriggerHit<'_>],
        listed: usize,
    ) -> Self {
        let wanted: HashSet<String> = hits
            .iter()
            .map(|hit| hit.trigger().name().to_ascii_lowercase())
            .collect();
        let index = TriggerUsers::build_positioned(forge, &|name| wanted.contains(name));
        Self {
            forge,
            index,
            listed,
        }
    }

    pub(super) const fn unreadable_records(&self) -> usize {
        self.index.unreadable_records()
    }

    pub(super) const fn index(&self) -> &TriggerUsers<Option<TriggerOccurrence>> {
        &self.index
    }

    pub(super) const fn record_count(&self) -> usize {
        self.forge.record_count()
    }

    pub(super) fn observed(
        &self,
        trigger: &str,
    ) -> impl Iterator<Item = (Record<'forge>, Option<&TriggerOccurrence>)> {
        self.index
            .observed_entities(self.forge, trigger)
            .map(|(record, use_)| (record, use_.as_ref()))
    }

    pub(super) fn shape(&self, name: &str, text: &mut Budget) -> Result<EntitiesShape, ToolError> {
        Ok(EntitiesShape {
            count: self.index.count(name),
            listed: self
                .index
                .entities(self.forge, name)
                .take(self.listed)
                .map(|record| header::reference(&record, text))
                .collect::<Result<_, _>>()?,
        })
    }
}
