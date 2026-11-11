// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, TryReserveError};

use p4kx_extraction::bank::BankReadError;
use schemars::JsonSchema;
use serde::Serialize;

use crate::MediaError;
use crate::audio_bank_projection::BankProjection;

pub(crate) struct BankCandidateIndex<T> {
    candidates: HashMap<u32, Vec<T>>,
    incomplete: bool,
}

pub(crate) struct Observation<T> {
    pub(crate) id: u32,
    pub(crate) candidate: T,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Resolution {
    Resolved,
    Missing,
    Ambiguous,
    Unavailable,
}

impl Resolution {
    pub(crate) const fn from_inventory(complete: bool, matches: u64) -> Self {
        match (complete, matches) {
            (false, _) => Self::Unavailable,
            (true, 0) => Self::Missing,
            (true, 1) => Self::Resolved,
            (true, _) => Self::Ambiguous,
        }
    }
}

impl<T> Default for BankCandidateIndex<T> {
    fn default() -> Self {
        Self {
            candidates: HashMap::new(),
            incomplete: false,
        }
    }
}

impl<T> BankCandidateIndex<T> {
    pub(crate) fn observe_rows<S>(
        &mut self,
        projection: &BankProjection<'_>,
        rows: Result<impl IntoIterator<Item = Result<S, BankReadError>>, BankReadError>,
        mut observe: impl FnMut(S) -> Observation<T>,
    ) -> Result<(), MediaError> {
        let rows = match rows {
            Ok(rows) => rows,
            Err(BankReadError::Format(_)) => {
                self.mark_incomplete();
                return Ok(());
            }
            Err(cause) => return Err(projection.replay_failure(cause)),
        };
        for result in rows {
            match result {
                Ok(row) => self.insert(observe(row))?,
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

    pub(crate) const fn resolution(&self, matches: u64) -> Resolution {
        Resolution::from_inventory(self.is_complete(), matches)
    }

    pub(crate) fn candidates(&self, id: u32) -> &[T] {
        self.candidates.get(&id).map_or(&[], Vec::as_slice)
    }

    fn insert(&mut self, observation: Observation<T>) -> Result<(), MediaError> {
        let Observation { id, candidate } = observation;
        if let Some(candidates) = self.candidates.get_mut(&id) {
            candidates
                .try_reserve(1)
                .map_err(|cause| allocation(&cause))?;
            candidates.push(candidate);
        } else {
            self.candidates
                .try_reserve(1)
                .map_err(|cause| allocation(&cause))?;
            let mut candidates = Vec::new();
            candidates
                .try_reserve_exact(1)
                .map_err(|cause| allocation(&cause))?;
            candidates.push(candidate);
            self.candidates.insert(id, candidates);
        }
        Ok(())
    }
}

fn allocation(cause: &TryReserveError) -> MediaError {
    MediaError::Contract(format!("cannot allocate the bank candidate index: {cause}"))
}
