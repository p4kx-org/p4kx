// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;

use crate::audio_bank_origins::BankOrigins;
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaWriter};

use super::{List, MusicSites, coordinates, field, row_route};

impl MusicSites<'_> {
    pub(crate) fn schedule_origins<'source, 'writer, 'run>(
        &self,
        input: &'source MediaOriginInput,
        ordinal: u64,
        range: &Range<u64>,
        malformed: bool,
        writer: &'writer MediaWriter<'run>,
    ) -> Result<BankOrigins<'source, 'writer, 'run>, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        self.row(&mut prepared, List::Stingers, ordinal)?;
        let indices = self.indices(ordinal);
        let span = coordinates(range);
        let selection = prepared.capture(self.list_route(List::Stingers).chain([
            row_route(self.namespace, List::Stingers, &indices),
            field(&span, "musicStingerScheduleExtent", SourceUseRole::Selector),
        ]))?;
        prepared.field("/subresource/indices/2", selection.clone())?;
        prepared.field("/data/payload/layout", selection.clone())?;
        if malformed {
            prepared.subtree("/issues", selection)?;
        }
        Ok(prepared)
    }

    pub(crate) fn schedule_word(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        ordinal: u64,
        key: &str,
        label: &'static str,
        range: &Range<u64>,
    ) -> Result<(), MediaError> {
        self.row_word(prepared, List::Stingers, ordinal, key, label, range)
    }

    pub(crate) fn schedule_signed(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        ordinal: u64,
        range: &Range<u64>,
    ) -> Result<(), MediaError> {
        let indices = self.indices(ordinal);
        let span = coordinates(range);
        let receipt = prepared.capture(self.list_route(List::Stingers).chain([
            row_route(self.namespace, List::Stingers, &indices),
            field(
                &span,
                "musicStingerDontRepeatSigned",
                SourceUseRole::Conversion,
            ),
        ]))?;
        prepared.field("/data/payload/dont_repeat_time/signed_value", receipt)
    }
}
