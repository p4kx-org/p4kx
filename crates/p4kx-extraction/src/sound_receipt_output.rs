// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_audio::{AudioError, Wem};

use super::{SoundArtifactSource, SoundOutput};
use crate::file_receipt::{self, StagedReceiptPublication, finish_staged_receipt};
use crate::origin_file_io::StageDisposition;
use crate::output_file::StagedOutput;
use crate::sound::{
    OggWrite, SoundEncodeError, SoundOrigins, WavWrite, capture_ogg, capture_raw_member,
    capture_wav,
};
use crate::texture_convert::Step;
use crate::tracker::ExtractionArtifacts;
use crate::write::{Destination, EntryWriter, write_error};
use crate::{
    ConflictPolicy, ConvertedSoundArtifact, ExtractionError, FileReceipt, OriginArtifactDigest,
    OriginDigestWriter, RawEntryArtifact,
};

enum SoundReceiptKind {
    Converted,
    RawMember,
}

impl SoundReceiptKind {
    fn track(
        self,
        tracker: &ExtractionArtifacts,
        artifact: &Path,
        path: String,
        origin_receipt: Option<String>,
        digest: OriginArtifactDigest,
    ) {
        match (self, origin_receipt) {
            (Self::Converted, Some(origin_receipt)) => tracker.insert_sound(
                artifact,
                ConvertedSoundArtifact {
                    path,
                    origin_receipt,
                    digest,
                },
            ),
            (Self::Converted, None) => {}
            (Self::RawMember, origin_receipt) => tracker.insert_raw(
                artifact,
                RawEntryArtifact {
                    path,
                    digest,
                    origin_receipt,
                },
            ),
        }
    }
}

impl EntryWriter<'_> {
    pub(crate) fn write_raw_bank_member(
        &self,
        destination: &Destination,
        bytes: &[u8],
        source: &SoundArtifactSource<'_>,
        chunk: &mut [u8],
    ) -> Result<SoundOutput, ExtractionError> {
        let length = bytes.len() as u64;
        self.write_sound_with_receipt(
            destination,
            source,
            chunk,
            SoundReceiptKind::RawMember,
            |sink| {
                sink.write_all(bytes).map_err(AudioError::Write)?;
                Ok(((), length))
            },
            |()| capture_raw_member(source.input, length),
        )?
        .map_err(|error| write_error(destination.file.clone(), io::Error::other(error)))
    }

    pub(crate) fn write_wav_sound(
        &self,
        destination: &Destination,
        wem: &Wem<&[u8]>,
        source: &SoundArtifactSource<'_>,
        chunk: &mut [u8],
    ) -> Result<Result<SoundOutput, AudioError>, ExtractionError> {
        self.write_sound_with_receipt(
            destination,
            source,
            chunk,
            SoundReceiptKind::Converted,
            |sink| {
                let written =
                    WavWrite::encode(wem, sink).map_err(SoundEncodeError::into_audio_error)?;
                let size = written.report().size;
                Ok((written, size))
            },
            |written| capture_wav(wem, source.input, &written),
        )
    }

    pub(crate) fn write_ogg_sound(
        &self,
        destination: &Destination,
        wem: &Wem<&[u8]>,
        source: &SoundArtifactSource<'_>,
        chunk: &mut [u8],
    ) -> Result<Result<SoundOutput, AudioError>, ExtractionError> {
        self.write_sound_with_receipt(
            destination,
            source,
            chunk,
            SoundReceiptKind::Converted,
            |sink| {
                let written =
                    OggWrite::encode(wem, sink).map_err(SoundEncodeError::into_audio_error)?;
                let size = written.report().size;
                Ok((written, size))
            },
            |written| capture_ogg(wem, source.input, &written),
        )
    }

    fn write_sound_with_receipt<T>(
        &self,
        destination: &Destination,
        source: &SoundArtifactSource<'_>,
        chunk: &mut [u8],
        kind: SoundReceiptKind,
        encode: impl FnOnce(&mut dyn Write) -> Result<(T, u64), AudioError>,
        capture: impl FnOnce(T) -> io::Result<SoundOrigins>,
    ) -> Result<Result<SoundOutput, AudioError>, ExtractionError> {
        self.stage_sound(
            destination,
            |sink| {
                let mut measured = OriginDigestWriter::new(sink);
                let (written, size) = encode(&mut measured)?;
                let digest = measured.finish().map_err(AudioError::Write)?;
                if digest.byte_length() != size {
                    return Err(AudioError::Write(io::Error::other(
                        "audio byte length differs from its write report",
                    )));
                }
                Ok(MeasuredSound {
                    digest,
                    written,
                    kind,
                })
            },
            |staged, output| self.publish_sound_receipt(staged, source, output, capture),
            chunk,
        )
    }

    fn publish_sound_receipt<T>(
        &self,
        staged: &StagedOutput<'_>,
        source: &SoundArtifactSource<'_>,
        output: MeasuredSound<T>,
        capture: impl FnOnce(T) -> io::Result<SoundOrigins>,
    ) -> Result<StageDisposition<SoundOutput>, ExtractionError> {
        let artifact = staged.output();
        let relative = artifact
            .strip_prefix(self.target)
            .map_err(|_| ExtractionError::UnsafePath)?;
        let companion = file_receipt::companion_path(relative)
            .map_err(|error| write_error(artifact.to_path_buf(), error))?;
        let receipt_path = self.target.join(&companion);
        if !source.is_embedded {
            self.case_collision(relative, &source.entry)?;
        }
        self.case_collision(&companion, &source.entry)?;
        crate::portable_name::check_path_length(&receipt_path)?;

        let report_path = relative_path(relative)?;
        let receipt_name = relative_path(&companion)?;
        let report_digest = output.digest.clone();
        let origins =
            capture(output.written).map_err(|error| write_error(receipt_path.clone(), error))?;
        let receipt = FileReceipt::for_staged_replacement(
            artifact,
            self.protection,
            self.cancel.cloned().unwrap_or_default(),
        )
        .map_err(|error| write_error(receipt_path.clone(), error))?;
        match receipt.publish_staged_with_policy(
            staged,
            output.digest,
            self.policy,
            || self.receipt_checkpoint(),
            || self.tracker.artifacts.invalidate(artifact),
            |writer, binding| origins.write(writer, binding),
        ) {
            Some(StagedReceiptPublication::Uncommitted(error)) => {
                Err(write_error(artifact.to_path_buf(), error))
            }
            None => Ok(StageDisposition::Cleanup(SoundOutput::of(Step::Kept))),
            Some(StagedReceiptPublication::Committed {
                receipt,
                retirement,
            }) => {
                source.record_claimed_path(self.tracker, relative);
                let companion_written = receipt.as_ref().is_some_and(Result::is_ok);
                if companion_written {
                    source.record_claimed_path(self.tracker, &companion);
                }
                output.kind.track(
                    &self.tracker.artifacts,
                    artifact,
                    report_path,
                    companion_written.then_some(receipt_name),
                    report_digest,
                );
                Ok(finish_sound_publication(
                    self.policy,
                    receipt_path,
                    receipt,
                    retirement,
                ))
            }
        }
    }
}

fn finish_sound_publication(
    policy: ConflictPolicy,
    receipt_path: PathBuf,
    receipt: Option<io::Result<()>>,
    retirement: io::Result<()>,
) -> StageDisposition<SoundOutput> {
    let receipt_failure = finish_staged_receipt(receipt, retirement)
        .map_err(|error| write_error(receipt_path, error))
        .err();
    let output = SoundOutput {
        step: Step::Written,
        receipt_failure,
    };
    if policy == ConflictPolicy::Skip {
        StageDisposition::Cleanup(output)
    } else {
        StageDisposition::Moved(output)
    }
}

struct MeasuredSound<T> {
    digest: OriginArtifactDigest,
    written: T,
    kind: SoundReceiptKind,
}

fn relative_path(path: &Path) -> Result<String, ExtractionError> {
    path.to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or(ExtractionError::UnsafePath)
}
