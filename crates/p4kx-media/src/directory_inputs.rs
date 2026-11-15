// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::OnceCell;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use crate::checkpoint::Checkpoint;
use crate::input_budget::InputBudget;
use crate::{Limits, MediaError, MediaWriter, RawReference};

#[derive(Default)]
struct Slot {
    busy: bool,
    raw: Option<RawReference>,
}

pub(crate) struct DirectoryInputs<'run> {
    pub(crate) budget: &'run InputBudget,
    pub(crate) animation_skeleton: OnceCell<crate::animation::SkeletonSelection>,
    pub(crate) definition_scope: OnceCell<crate::animation_definition::DefinitionScope>,
    slots: Mutex<HashMap<usize, Slot>>,
    count: usize,
}

/// Shared physical Entry admission for readers that do not use a Media writer.
#[derive(Clone, Copy)]
pub(crate) struct DirectoryAdmission<'run> {
    budget: &'run InputBudget,
    slots: &'run Mutex<HashMap<usize, Slot>>,
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
            definition_scope: OnceCell::new(),
            slots: Mutex::default(),
            count,
        })
    }

    pub(crate) const fn admission(&self) -> DirectoryAdmission<'_> {
        DirectoryAdmission {
            budget: self.budget,
            slots: &self.slots,
            count: self.count,
        }
    }

    pub(crate) fn source_allowance(&self, limit: u64) -> Result<u64, MediaError> {
        let slots = self.admission().slots()?;
        Ok(limit.saturating_sub(self.budget.source_count()?) + slots.len() as u64)
    }

    pub(crate) fn admit(
        &self,
        position: usize,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        self.admission()
            .admit(position, writer.limits(), writer.checkpoint, &writer.root)
    }

    pub(crate) fn begin(
        &self,
        position: usize,
        writer: &MediaWriter<'_>,
    ) -> Result<Option<RawReference>, MediaError> {
        self.admit(position, writer)?;
        let mut slots = self.admission().slots()?;
        let slot = slots.get_mut(&position).ok_or_else(invalid_position)?;
        if slot.busy {
            return Err(busy());
        }
        slot.busy = true;
        let raw = slot.raw.clone();
        drop(slots);
        Ok(raw)
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
        let mut slots = self.admission().slots()?;
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
        drop(slots);
        Ok(Some(raw))
    }

    pub(crate) fn end(&self, position: usize, raw: Option<RawReference>) -> Result<(), MediaError> {
        let mut slots = self.admission().slots()?;
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
        drop(slots);
        Ok(())
    }
}

impl<'run> DirectoryAdmission<'run> {
    pub(crate) fn admit(
        &self,
        position: usize,
        limits: Limits,
        checkpoint: Checkpoint<'_>,
        output_root: &Path,
    ) -> Result<(), MediaError> {
        checkpoint.check()?;
        if position >= self.count {
            return Err(invalid_position());
        }
        let mut slots = self.slots()?;
        if slots.contains_key(&position) {
            return Ok(());
        }
        self.budget.check_source(limits)?;
        slots.try_reserve(1).map_err(|cause| MediaError::Io {
            path: output_root.to_path_buf(),
            source: io::Error::new(io::ErrorKind::OutOfMemory, cause),
        })?;
        checkpoint.check()?;
        self.budget.admit_source(limits)?;
        slots.insert(position, Slot::default());
        drop(slots);
        Ok(())
    }

    fn slots(&self) -> Result<MutexGuard<'run, HashMap<usize, Slot>>, MediaError> {
        self.slots
            .lock()
            .map_err(|_| MediaError::Contract("directory input mutex is poisoned".into()))
    }
}

fn busy() -> MediaError {
    MediaError::Contract("required input is already active".into())
}

fn invalid_position() -> MediaError {
    MediaError::Contract("required input has no physical directory position".into())
}

#[cfg(test)]
mod tests {
    use std::sync::Barrier;
    use std::thread;

    use p4kx_extraction::Exclusion;
    use p4kx_testkit::TestEntry;

    use crate::required_input_fault_support::{Inventory, TestResult, with_fixture};
    use crate::{Limits, MediaError};

    #[test]
    fn concurrent_admission_of_one_position_charges_one_source() -> TestResult {
        with_fixture(
            [
                TestEntry::stored("Data/a.dds", b""),
                TestEntry::stored("Data/b.dds", b""),
                TestEntry::stored("Data/c.dds", b""),
            ],
            Inventory::Intact,
            &Exclusion::default(),
            |context, writer, checkpoint, output_root| {
                let inputs = context.directory.input_state()?;
                let admission = inputs.admission();
                let limits = Limits {
                    sources: 2,
                    ..writer.limits()
                };
                let start = Barrier::new(2);
                thread::scope(|scope| -> TestResult {
                    let worker = scope.spawn(|| {
                        start.wait();
                        admission.admit(1, limits, checkpoint, output_root)
                    });
                    start.wait();
                    let local = admission.admit(1, limits, checkpoint, output_root);
                    worker.join().map_err(|_| "admission worker panicked")??;
                    local?;
                    Ok(())
                })?;
                assert_eq!(inputs.budget.source_count()?, 2, "owner plus shared Entry");
                assert!(matches!(
                    admission.admit(2, limits, checkpoint, output_root),
                    Err(MediaError::Limit {
                        budget: "sources",
                        limit: 2
                    })
                ));
                Ok(())
            },
        )
    }
}
