// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_extraction::bank::{BankReadError, BankSourceSection};
use p4kx_extraction::{BankError, DecimalU64, InputScope, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_origins::{BankOrigins, section_routing};
use crate::audio_bank_payload::BankPayload;
use crate::audio_bank_projection::BankProjection;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource, error};

pub(crate) const FAILURE_NAMESPACE: &str = "audio-bank-failure";
const BNK_TAG_BYTES: u64 = 4;

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum BankPhase {
    Signature,
    Directory,
    Header,
    MediaIndex,
    Hierarchy,
    Names,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BankFailureRecord {
    phase: BankPhase,
    section_ordinal: Option<DecimalU64>,
    reason: BankFailureReason,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
enum BankFailureReason {
    NotSoundbank,
    UnsupportedVersion {
        version: u32,
    },
    UnsupportedObjectLayout {
        version: u32,
        kind: &'static str,
    },
    Truncated {
        section: String,
        available: DecimalU64,
        needed: DecimalU64,
    },
    InvalidSection {
        section: String,
        detail: &'static str,
    },
    DuplicateSection {
        section: String,
    },
    MediaOutsideData {
        id: u32,
        offset: DecimalU64,
        end: DecimalU64,
        data_length: DecimalU64,
    },
}

impl BankProjection<'_> {
    pub(crate) fn failure(
        &self,
        phase: BankPhase,
        section: Option<&BankSourceSection>,
        cause: BankReadError,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        match cause {
            BankReadError::Format(cause) => self.format_failure(phase, section, &cause, writer),
            cause => Err(self.replay_failure(cause)),
        }
    }

    pub(crate) fn replay_failure(&self, cause: BankReadError) -> MediaError {
        let cause = match cause {
            BankReadError::Io(cause) => cause,
            other => io::Error::new(io::ErrorKind::InvalidData, other),
        };
        error::io(Path::new(self.path), cause)
    }

    fn format_failure(
        &self,
        phase: BankPhase,
        section: Option<&BankSourceSection>,
        cause: &BankError,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let (phase_ordinal, location) = phase.location();
        let section_ordinal = section.map(BankSourceSection::ordinal);
        let mut indices = vec![phase_ordinal];
        indices.extend(section_ordinal);
        let mut prepared = BankOrigins::new(self.input, writer)?;
        let evidence = self.observe_failure(phase, section, location, writer)?;
        prepared.subtree("/data/payload/reason", evidence.clone())?;
        prepared.subtree("/issues", evidence)?;
        let location_evidence = match section_ordinal {
            Some(ordinal) => prepared.capture([section_routing(&[ordinal])])?,
            None => OriginContributions::independent(&self.input.owner),
        };
        prepared.field("/data/payload/section_ordinal", location_evidence.clone())?;
        prepared.indices(location_evidence)?;
        prepared.field(
            "/subresource/indices/0",
            OriginContributions::independent(&self.input.owner),
        )?;
        let payload = BankPayload::BankFailure(BankFailureRecord {
            phase,
            section_ordinal: section_ordinal.map(DecimalU64::new),
            reason: BankFailureReason::from(cause),
        });
        prepared.finish().write(
            writer,
            &Subresource::new(FAILURE_NAMESPACE, indices),
            Status::Failed,
            &payload,
            &[MediaIssue::new(
                "audio.bank.invalid-structure",
                location,
                Severity::Error,
                cause.to_string(),
            )],
        )
    }
    fn observe_failure(
        &self,
        phase: BankPhase,
        section: Option<&BankSourceSection>,
        location: &str,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        if matches!(phase, BankPhase::Signature) && self.length < BNK_TAG_BYTES {
            capture.add(
                self.input,
                InputScope::ArchiveDirectoryMetadata,
                OriginSite::new("archive-directory", &[], "uncompressedSize"),
                SourceUseRole::Conversion,
                writer,
            )?;
        } else {
            capture.include(&OriginContributions::unknown(&self.input.owner), writer)?;
            let coordinate = section.map(BankSourceSection::body_offset);
            capture.add(
                self.input,
                InputScope::Payload,
                OriginSite::new("bnkReadFailure", coordinate.as_slice(), location),
                SourceUseRole::Conversion,
                writer,
            )?;
        }
        Ok(capture.finish())
    }
}

impl BankPhase {
    const fn location(self) -> (u64, &'static str) {
        match self {
            Self::Signature => (0, "signature"),
            Self::Directory => (1, "section-directory"),
            Self::Header => (2, "BKHD"),
            Self::MediaIndex => (3, "DIDX"),
            Self::Hierarchy => (4, "HIRC"),
            Self::Names => (5, "STID"),
        }
    }
}

impl From<&BankError> for BankFailureReason {
    fn from(cause: &BankError) -> Self {
        match cause {
            BankError::NotSoundbank => Self::NotSoundbank,
            BankError::UnsupportedVersion { version } => {
                Self::UnsupportedVersion { version: *version }
            }
            BankError::UnsupportedObjectLayout { kind, version } => Self::UnsupportedObjectLayout {
                version: *version,
                kind: kind.slug(),
            },
            BankError::Truncated {
                section,
                len,
                needed,
            } => Self::Truncated {
                section: section.to_string(),
                available: DecimalU64::new(*len),
                needed: DecimalU64::new(*needed),
            },
            BankError::InvalidSection { section, reason } => Self::InvalidSection {
                section: section.to_string(),
                detail: reason,
            },
            BankError::DuplicateSection(section) => Self::DuplicateSection {
                section: section.to_string(),
            },
            BankError::MediaOutsideData {
                id,
                offset,
                end,
                data_len,
            } => Self::MediaOutsideData {
                id: *id,
                offset: DecimalU64::new(*offset),
                end: DecimalU64::new(*end),
                data_length: DecimalU64::new(*data_len),
            },
        }
    }
}
