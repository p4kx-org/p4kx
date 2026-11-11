// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::BankSourceSection;
use p4kx_extraction::{BankHeader, DecimalU64};

use crate::audio_bank_payload::{BankEntryRange, BankHeaderRecord, BankPayload, BankSectionRecord};
use crate::origin_input::MediaOriginInput;
use crate::{
    MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource, audio_bank_origins,
};

pub(crate) const HEADER_NAMESPACE: &str = "audio-bank";
pub(crate) const SECTION_NAMESPACE: &str = "audio-bank-section";

pub(crate) struct BankProjection<'source> {
    pub(crate) input: &'source MediaOriginInput,
    pub(crate) path: &'source str,
    pub(crate) length: u64,
}

impl BankProjection<'_> {
    pub(crate) fn section(
        &self,
        section: &BankSourceSection,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let duplicate = section.is_duplicate_unique();
        let payload = BankPayload::BankSection(BankSectionRecord {
            ordinal: DecimalU64::new(section.ordinal()),
            tag: section.id().0,
            header_range: BankEntryRange::new(section.header_offset(), section.body_offset()),
            body_range: BankEntryRange::new(section.body_offset(), section.body_end_exclusive()),
            duplicate_unique: duplicate,
        });
        let (status, issues) = if duplicate {
            (
                Status::Partial,
                Some(MediaIssue::new(
                    "audio.bank.duplicate-section",
                    "section-directory",
                    Severity::Warning,
                    format!("Soundbank section {} appears more than once", section.id()),
                )),
            )
        } else {
            (Status::Complete, None)
        };
        audio_bank_origins::section(self.input, section, writer)?.write(
            writer,
            &Subresource::new(SECTION_NAMESPACE, vec![section.ordinal()]),
            status,
            &payload,
            issues.as_slice(),
        )
    }

    pub(crate) fn header(
        &self,
        section: &BankSourceSection,
        header: &BankHeader,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let supported = header.is_supported_version();
        let payload = BankPayload::Bank(BankHeaderRecord {
            section_ordinal: DecimalU64::new(section.ordinal()),
            version: header.version,
            version_supported: supported,
            bank_id: header.id,
            language_id: header.language_id,
        });
        let issue = (!supported).then(|| {
            MediaIssue::new(
                "audio.bank.unsupported-version",
                "BKHD",
                Severity::Info,
                format!(
                    "Soundbank version {} has no supported layout",
                    header.version
                ),
            )
        });
        audio_bank_origins::header(self.input, section, supported, writer)?.write(
            writer,
            &Subresource::new(HEADER_NAMESPACE, vec![section.ordinal()]),
            Status::Complete,
            &payload,
            issue.as_slice(),
        )
    }
}
