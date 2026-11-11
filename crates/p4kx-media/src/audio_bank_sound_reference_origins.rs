// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;

use crate::audio_bank_didx_index::DidxSite;

use crate::audio_bank_origins::{BankOrigins, complete_directory, section_routing};
use crate::audio_bank_sound_origins::SoundSites;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

impl SoundSites {
    pub(super) fn didx_summary(
        &self,
        input: &MediaOriginInput,
        complete: bool,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let routing = prepared.capture([self.route()])?;
        prepared.indices(routing)?;
        let observed = prepared.capture(
            self.base()
                .into_iter()
                .chain(self.media_selectors()?)
                .chain([
                    complete_directory("didxSections"),
                    (
                        OriginSite::new("bnkDidxInventory", &[], "observedMediaIds"),
                        SourceUseRole::Selector,
                    ),
                ]),
        )?;
        prepared.field("/data/payload/observed_candidates", observed.clone())?;
        prepared.field("/data/payload/candidates_complete", observed.clone())?;
        let resolution = if complete {
            observed
        } else {
            let mut capture = writer.capture_origins();
            capture.include(&observed, writer)?;
            capture.include(&OriginContributions::unknown(&input.owner), writer)?;
            capture.finish()
        };
        prepared.field("/data/payload/resolution", resolution)?;
        Ok(prepared.finish())
    }

    pub(super) fn didx_candidate(
        &self,
        input: &MediaOriginInput,
        target: &DidxSite,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let section = [target.section];
        let indices = [target.section, target.triplet];
        let offset = [target.offset];
        let link = prepared.capture(
            self.base()
                .into_iter()
                .chain(self.media_selectors()?)
                .chain([
                    section_routing(&section),
                    (
                        OriginSite::new("bnkDidxPrefix", &indices, "tripletRouting"),
                        SourceUseRole::Selector,
                    ),
                    (
                        OriginSite::new("bnkDidx", &offset, "mediaId"),
                        SourceUseRole::Selector,
                    ),
                ]),
        )?;
        prepared.indices(link.clone())?;
        let sound = prepared.capture([self.route()])?;
        prepared.field("/subresource/indices/0", sound.clone())?;
        prepared.field("/subresource/indices/1", sound)?;
        prepared.field("/data/payload/target_section_ordinal", link.clone())?;
        prepared.field("/data/payload/target_triplet_ordinal", link)?;
        Ok(prepared.finish())
    }
}
