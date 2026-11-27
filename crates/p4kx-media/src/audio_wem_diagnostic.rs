// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::sound::WemReadError;
use p4kx_extraction::{AudioError, DecimalU64, InputScope, SourceUseRole};

use crate::audio_payload::{
    AudioPayload, WemFailure, WemFailureReason, WemOpaque, WemOpaqueClass, WemPhase,
};
use crate::audio_record_origins as origins;
use crate::audio_wem_projection::WemProjection;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, error};

pub(crate) const OPAQUE_NAMESPACE: &str = "audio-wem-opaque";
pub(crate) const FAILURE_NAMESPACE: &str = "audio-wem-failure";

const RIFF_SIGNATURE_BYTES: u64 = 4;

impl WemProjection<'_> {
    pub(crate) fn opaque(
        &self,
        class: WemOpaqueClass,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        if self.is_embedded() {
            return Err(MediaError::Contract(
                "selected WEM has a different class when replayed".into(),
            ));
        }
        let mut prepared = origins::prepare(self.input)?;
        let observed = self.capture(
            [(
                OriginSite::new("wemRiffHeader", &[], "mediaClassSelector"),
                SourceUseRole::Conversion,
            )],
            writer,
        )?;
        origins::field(&mut prepared, "/data/payload/class", observed, writer)?;
        self.indices(
            &mut prepared,
            OriginContributions::independent(&self.input.owner),
            writer,
        )?;
        prepared.write(
            writer,
            &self.subresource(OPAQUE_NAMESPACE, &[])?,
            Status::Complete,
            &AudioPayload::WemOpaque(WemOpaque { class }),
            &[],
        )
    }

    pub(crate) fn read_failure(
        &self,
        phase: WemPhase,
        cause: WemReadError,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        match cause {
            WemReadError::Format(cause) => self.format_failure(phase, &cause, writer),
            WemReadError::Io(cause) => Err(error::io(Path::new(self.path), cause)),
            other => Err(MediaError::Contract(format!(
                "unrecognized WEM reader failure: {other}"
            ))),
        }
    }

    fn format_failure(
        &self,
        phase: WemPhase,
        cause: &AudioError,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut prepared = origins::prepare(self.input)?;
        let (position, location) = phase_location(phase);
        let mut capture = writer.capture_origins();
        let length_only = !self.is_embedded()
            && matches!(phase, WemPhase::RiffHeader)
            && self.byte_length < RIFF_SIGNATURE_BYTES;
        if length_only {
            capture.add(
                self.input,
                InputScope::ArchiveDirectoryMetadata,
                OriginSite::new("archive-directory", &[], "uncompressedSize"),
                SourceUseRole::Conversion,
                writer,
            )?;
        } else {
            capture.include(&OriginContributions::unknown(&self.input.owner), writer)?;
            let observed = self.capture(
                [(
                    OriginSite::new("wemReadFailure", &[], location),
                    SourceUseRole::Conversion,
                )],
                writer,
            )?;
            capture.include(&observed, writer)?;
        }
        let evidence = capture.finish();
        origins::subtree(
            &mut prepared,
            "/data/payload/reason",
            evidence.clone(),
            writer,
        )?;
        origins::subtree(&mut prepared, "/issues", evidence, writer)?;
        self.indices(
            &mut prepared,
            OriginContributions::independent(&self.input.owner),
            writer,
        )?;
        prepared.write(
            writer,
            &self.subresource(FAILURE_NAMESPACE, &[position])?,
            Status::Failed,
            &AudioPayload::WemFailure(WemFailure {
                phase,
                reason: reason(cause),
            }),
            &[MediaIssue::new(
                "audio.wem.invalid-structure",
                location,
                Severity::Error,
                cause.to_string(),
            )],
        )
    }
}

const fn phase_location(phase: WemPhase) -> (u64, &'static str) {
    match phase {
        WemPhase::RiffHeader => (0, "riff-header"),
        WemPhase::Summary => (1, "summary"),
        WemPhase::ChunkDirectory => (2, "chunk-directory"),
    }
}

pub(crate) fn reason(cause: &AudioError) -> WemFailureReason {
    match cause {
        AudioError::NotWem => WemFailureReason::InvalidSignature,
        AudioError::Truncated { len, needed } => WemFailureReason::Truncated {
            available: DecimalU64::new(*len),
            needed: DecimalU64::new(*needed),
        },
        AudioError::MissingChunk(name) => WemFailureReason::MissingChunk { name },
        AudioError::InvalidFormat(detail) => WemFailureReason::InvalidFormat { detail },
        other => WemFailureReason::UnsupportedReaderVariant {
            detail: other.to_string(),
        },
    }
}
