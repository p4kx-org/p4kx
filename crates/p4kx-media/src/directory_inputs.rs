// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io;

use crate::input_budget::InputBudget;
use crate::{MediaError, MediaWriter, RawReference};

#[derive(Default)]
struct Slot {
    busy: bool,
    raw: Option<RawReference>,
}

pub(crate) struct DirectoryInputs<'run> {
    pub(crate) budget: &'run InputBudget,
    pub(crate) animation_skeleton: OnceCell<crate::animation::SkeletonSelection>,
    slots: RefCell<HashMap<usize, Slot>>,
    admitted: Cell<u64>,
    count: usize,
}

impl<'run> DirectoryInputs<'run> {
    pub(crate) fn try_new(
        count: usize,
        budget: &'run InputBudget,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        writer.checkpoint()?;
        if let Some(last) = count.checked_sub(1) {
            u32::try_from(last).map_err(|_| {
                MediaError::Contract("input position exceeds physical index width".into())
            })?;
        }
        Ok(Self {
            budget,
            animation_skeleton: OnceCell::new(),
            slots: RefCell::new(HashMap::new()),
            admitted: Cell::new(0),
            count,
        })
    }

    #[must_use]
    pub(crate) const fn source_allowance(&self, limit: u64) -> u64 {
        limit.saturating_sub(self.budget.sources.get()) + self.admitted.get()
    }

    pub(crate) fn admit(
        &self,
        position: usize,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        writer.checkpoint()?;
        if position >= self.count {
            return Err(invalid_position());
        }
        {
            let mut slots = self.slots.try_borrow_mut().map_err(|_| busy())?;
            if slots.contains_key(&position) {
                return Ok(());
            }
            self.budget.check_source(writer.limits())?;
            slots.try_reserve(1).map_err(|cause| MediaError::Io {
                path: writer.root.clone(),
                source: io::Error::new(io::ErrorKind::OutOfMemory, cause),
            })?;
        }
        writer.checkpoint()?;
        let mut slots = self.slots.try_borrow_mut().map_err(|_| busy())?;
        if let Entry::Vacant(slot) = slots.entry(position) {
            self.budget.admit_source(writer.limits())?;
            slot.insert(Slot::default());
            self.admitted.set(self.admitted.get() + 1);
        }
        Ok(())
    }

    pub(crate) fn begin(
        &self,
        position: usize,
        writer: &MediaWriter<'_>,
    ) -> Result<Option<RawReference>, MediaError> {
        self.admit(position, writer)?;
        let mut slots = self.slots.try_borrow_mut().map_err(|_| busy())?;
        let slot = slots.get_mut(&position).ok_or_else(invalid_position)?;
        if slot.busy {
            return Err(busy());
        }
        slot.busy = true;
        Ok(slot.raw.clone())
    }

    pub(crate) fn begin_retained(
        &self,
        position: usize,
        writer: &MediaWriter<'_>,
    ) -> Result<Option<RawReference>, MediaError> {
        writer.checkpoint()?;
        if position >= self.count {
            return Err(invalid_position());
        }
        let mut slots = self.slots.try_borrow_mut().map_err(|_| busy())?;
        let Some(slot) = slots.get_mut(&position) else {
            return Ok(None);
        };
        if slot.busy {
            return Err(busy());
        }
        let Some(raw) = slot.raw.clone() else {
            return Ok(None);
        };
        slot.busy = true;
        Ok(Some(raw))
    }

    pub(crate) fn end(&self, position: usize, raw: Option<RawReference>) -> Result<(), MediaError> {
        let mut slots = self.slots.try_borrow_mut().map_err(|_| busy())?;
        let slot = slots.get_mut(&position).ok_or_else(invalid_position)?;
        if !slot.busy {
            return Err(MediaError::Contract(
                "input operation was not active".into(),
            ));
        }
        slot.busy = false;
        if raw.is_some() {
            slot.raw = raw;
        }
        Ok(())
    }
}

fn busy() -> MediaError {
    MediaError::Contract("required input is already active".into())
}

fn invalid_position() -> MediaError {
    MediaError::Contract("required input has no physical directory position".into())
}
