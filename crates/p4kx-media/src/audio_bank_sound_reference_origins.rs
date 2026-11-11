// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::BankHircSound;

use crate::audio_bank_didx_index::DidxSite;
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_origins::{BankOrigins, EvidenceSite, complete_directory, section_routing};
use crate::audio_bank_sound_origins::{Scalar, SoundSites};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(super) struct SoundReferenceSites {
    sound: SoundSites,
    source_id: Scalar,
    storage: Scalar,
}

impl SoundReferenceSites {
    pub(super) fn new(sound: &BankHircSound, version: VersionSite) -> Self {
        Self {
            sound: SoundSites::new(sound, version),
            source_id: Scalar::new(sound.fields.source_id.as_ref(), "soundSourceId"),
            storage: Scalar::new(sound.fields.storage_code.as_ref(), "soundStorage"),
        }
    }

    pub(super) fn summary(
        &self,
        input: &MediaOriginInput,
        complete: bool,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let routing = prepared.capture([self.sound.route()])?;
        prepared.indices(routing)?;
        let observed = prepared.capture(
            self.sound
                .base()
                .into_iter()
                .chain(self.selection()?)
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

    pub(super) fn candidate(
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
            self.sound
                .base()
                .into_iter()
                .chain(self.selection()?)
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
        let sound = prepared.capture([self.sound.route()])?;
        prepared.field("/subresource/indices/0", sound.clone())?;
        prepared.field("/subresource/indices/1", sound)?;
        prepared.field("/data/payload/target_section_ordinal", link.clone())?;
        prepared.field("/data/payload/target_triplet_ordinal", link)?;
        Ok(prepared.finish())
    }

    fn selection(&self) -> Result<[EvidenceSite<'_>; 3], MediaError> {
        let [Some(id), Some(storage), Some(plugin)] =
            [&self.source_id, &self.storage, &self.sound.plugin]
                .map(|word| word.site(SourceUseRole::Selector))
        else {
            return Err(MediaError::Contract(
                "decoded Sound media reference lacks original selector words".into(),
            ));
        };
        Ok([id, storage, plugin])
    }
}
