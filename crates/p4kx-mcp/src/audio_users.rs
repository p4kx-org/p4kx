// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_assembly::audio::TriggerUsers;
use p4kx_extraction::DataForge;

use crate::audio_shapes::EntitiesShape;
use crate::budget::Budget;
use crate::error::ToolError;

use super::header;
use super::hit_plan::HitPlan;

pub(super) struct Users<'forge> {
    forge: &'forge DataForge<'forge>,
    index: TriggerUsers,
    listed: usize,
}

impl<'forge> Users<'forge> {
    pub(super) fn new(forge: &'forge DataForge<'forge>, hits: &[HitPlan], listed: usize) -> Self {
        let wanted: HashSet<String> = hits
            .iter()
            .map(|hit| hit.name.to_ascii_lowercase())
            .collect();
        let index = TriggerUsers::build(forge, &|name| wanted.contains(name));
        Self {
            forge,
            index,
            listed,
        }
    }

    pub(super) const fn unreadable_records(&self) -> usize {
        self.index.unreadable_records()
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
