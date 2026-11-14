// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{InputScope, SourceUseRole};

use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_sound_origins::SoundSites;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, origin_lookup};

pub(super) struct LooseReferenceOrigins<'source> {
    input: &'source MediaOriginInput,
    route: OriginContributions,
    media_id: OriginContributions,
    authored: OriginContributions,
    directory: OriginContributions,
}

impl<'source> LooseReferenceOrigins<'source> {
    pub(super) fn new(
        input: &'source MediaOriginInput,
        sites: &SoundSites,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let prepared = BankOrigins::new(input, writer)?;
        let route = prepared.capture([sites.route()])?;
        let media_id = prepared.capture([sites.media_id()?])?;
        let authored =
            prepared.capture(sites.base().into_iter().chain(sites.media_selectors()?))?;
        let directory = writer.origins.directory().ok_or_else(|| {
            MediaError::Contract("loose WEM reference lacks its owning Archive origin".into())
        })?;
        Ok(Self {
            input,
            route,
            media_id,
            authored,
            directory: origin_lookup::directory(Some(directory), writer)?,
        })
    }

    pub(super) fn summary(
        &self,
        complete: bool,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(self.input, writer)?;
        prepared.indices(self.route.clone())?;
        prepared.field("/data/payload/media_id", self.media_id.clone())?;
        let mut capture = writer.capture_origins();
        capture.include(&self.authored, writer)?;
        capture.include(&self.directory, writer)?;
        let observed = capture.finish();
        prepared.field("/data/payload/observed_candidates", observed.clone())?;
        prepared.field("/data/payload/candidates_complete", observed.clone())?;
        prepared.subtree("/data/payload/unavailable_reason", observed.clone())?;
        let resolution = if complete {
            observed
        } else {
            let mut capture = writer.capture_origins();
            capture.include(&observed, writer)?;
            capture.include(&OriginContributions::unknown(&self.input.owner), writer)?;
            capture.finish()
        };
        prepared.field("/data/payload/resolution", resolution)?;
        Ok(prepared.finish())
    }

    pub(super) fn candidate(
        &self,
        target: &MediaOriginInput,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(self.input, writer)?;
        let mut capture = writer.capture_origins();
        capture.include(&self.authored, writer)?;
        for field in ["path", "entry_index"] {
            capture.add(
                target,
                InputScope::ArchiveDirectoryMetadata,
                OriginSite::new("archive-directory", &[], field),
                SourceUseRole::Selector,
                writer,
            )?;
        }
        let link = capture.finish();
        prepared.field("/data/payload/target_entry_index", link.clone())?;
        let mut capture = writer.capture_origins();
        capture.include(&link, writer)?;
        capture.include(&self.directory, writer)?;
        prepared.indices(capture.finish())?;
        prepared.field("/subresource/indices/0", self.route.clone())?;
        prepared.field("/subresource/indices/1", self.route.clone())?;
        Ok(prepared.finish())
    }
}
