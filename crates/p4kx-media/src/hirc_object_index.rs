// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankHircObject, BankSource, BankSourceSection};

use crate::MediaError;
use crate::audio_bank_projection::BankProjection;
use crate::bank_candidate_index::{BankCandidateIndex, Observation};

pub(crate) type HircObjectIndex = BankCandidateIndex<BankHircObject>;

impl HircObjectIndex {
    pub(crate) fn observe_hirc<R: Read + Seek>(
        &mut self,
        projection: &BankProjection<'_>,
        source: &mut BankSource<R>,
        section: &BankSourceSection,
    ) -> Result<(), MediaError> {
        self.observe_rows(projection, source.hirc(section), |object| Observation {
            id: object.id(),
            candidate: object,
        })
    }
}
