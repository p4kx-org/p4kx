// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::hash_map::Entry;
use std::collections::{HashMap, TryReserveError};
use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankHircObject, BankReadError, BankSource, BankSourceSection};

use crate::MediaError;
use crate::audio_bank_projection::BankProjection;

#[derive(Default)]
pub(crate) struct HircObjectIndex {
    objects: HashMap<u32, Vec<BankHircObject>>,
    incomplete: bool,
}

impl HircObjectIndex {
    pub(crate) fn observe<R: Read + Seek>(
        &mut self,
        projection: &BankProjection<'_>,
        source: &mut BankSource<R>,
        section: &BankSourceSection,
    ) -> Result<(), MediaError> {
        let objects = match source.hirc(section) {
            Ok(objects) => objects,
            Err(BankReadError::Format(_)) => {
                self.mark_incomplete();
                return Ok(());
            }
            Err(cause) => return Err(projection.replay_failure(cause)),
        };
        for result in objects {
            match result {
                Ok(object) => self.insert(object)?,
                Err(BankReadError::Format(_)) => {
                    self.mark_incomplete();
                    break;
                }
                Err(cause) => return Err(projection.replay_failure(cause)),
            }
        }
        Ok(())
    }

    pub(crate) const fn mark_incomplete(&mut self) {
        self.incomplete = true;
    }

    pub(crate) const fn is_complete(&self) -> bool {
        !self.incomplete
    }

    pub(crate) fn candidates(&self, id: u32) -> &[BankHircObject] {
        self.objects.get(&id).map_or(&[], Vec::as_slice)
    }

    fn insert(&mut self, object: BankHircObject) -> Result<(), MediaError> {
        self.objects
            .try_reserve(1)
            .map_err(|cause| allocation(&cause))?;
        let candidates = match self.objects.entry(object.id()) {
            Entry::Vacant(entry) => entry.insert(Vec::new()),
            Entry::Occupied(entry) => entry.into_mut(),
        };
        candidates
            .try_reserve(1)
            .map_err(|cause| allocation(&cause))?;
        candidates.push(object);
        Ok(())
    }
}

fn allocation(cause: &TryReserveError) -> MediaError {
    MediaError::Contract(format!("cannot allocate the HIRC object index: {cause}"))
}
