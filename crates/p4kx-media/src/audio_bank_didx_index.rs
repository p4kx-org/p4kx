// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankSource, BankSourceSection};

use crate::MediaError;
use crate::audio_bank_projection::BankProjection;
use crate::bank_candidate_index::{BankCandidateIndex, Observation};

pub(crate) type DidxIndex = BankCandidateIndex<DidxSite>;

pub(crate) struct DidxSite {
    pub(crate) section: u64,
    pub(crate) triplet: u64,
    pub(crate) offset: u64,
}

impl DidxIndex {
    pub(crate) fn observe_didx<R: Read + Seek>(
        &mut self,
        projection: &BankProjection<'_>,
        source: &mut BankSource<R>,
        section: &BankSourceSection,
    ) -> Result<(), MediaError> {
        self.observe_rows(projection, source.didx(section), |triplet| Observation {
            id: triplet.media_id(),
            candidate: DidxSite {
                section: triplet.section_ordinal(),
                triplet: triplet.ordinal(),
                offset: triplet.triplet_offset(),
            },
        })
    }
}
