// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use p4kx_archive::Checksum;
use p4kx_audio::{AudioError, EmbeddedWemSource};

use crate::counting::Counting;
use crate::origin_file_io::StageDisposition;
use crate::output_file::StagedOutput;
use crate::sound::SoundOriginInput;
use crate::source::SourceEntry;
use crate::texture_convert::Step;
use crate::texture_outputs::step_of;
use crate::tracker::Tracker;
use crate::write::{Destination, EntryWriter, Outcome, write_error};
use crate::{ConflictPolicy, ExtractionError, ObservedFileOrigin, OriginDigestWriter, SourceInput};

#[path = "sound_receipt_output.rs"]
mod receipt;

pub(crate) struct SoundArtifactSource<'a> {
    entry: SourceEntry<'a>,
    input: Option<SoundOriginInput<'a>>,
    is_embedded: bool,
}

impl<'a> SoundArtifactSource<'a> {
    pub(crate) fn loose(entry: SourceEntry<'a>, input: Option<SourceInput<'a>>) -> Self {
        Self {
            entry,
            input: input.map(SoundOriginInput::loose),
            is_embedded: false,
        }
    }

    pub(crate) fn embedded(
        entry: SourceEntry<'a>,
        input: Option<SourceInput<'a>>,
        member: EmbeddedWemSource,
    ) -> Self {
        Self {
            entry,
            input: input.map(|source| SoundOriginInput::embedded(source, member)),
            is_embedded: true,
        }
    }

    fn record_claimed_path(&self, tracker: &Tracker<'_>, path: &Path) {
        if !self.is_embedded {
            tracker.written.record(path, self.entry.index());
        }
    }
}

pub(crate) fn sound_input<'a>(
    entry: &SourceEntry<'a>,
    file: Option<&'a ObservedFileOrigin>,
) -> Option<SourceInput<'a>> {
    entry
        .observed_entry()
        .map(SourceInput::ArchiveView)
        .or_else(|| file.map(SourceInput::File))
}

pub(crate) struct SoundOutput {
    pub(crate) step: Step,
    pub(crate) receipt_failure: Option<ExtractionError>,
}

impl SoundOutput {
    fn from_raw_outcome(outcome: Outcome) -> Self {
        match outcome {
            Outcome::RawWritten(receipt_failure) => Self {
                step: Step::Written,
                receipt_failure,
            },
            outcome => Self::of(step_of(&outcome)),
        }
    }

    pub(crate) const fn of(step: Step) -> Self {
        Self {
            step,
            receipt_failure: None,
        }
    }
}

const WRITE_BUFFER_LEN: usize = 256 * 1024;

/// A converted file being written: counts its length and CRC-32C and fails once the run stops.
pub(crate) struct Sink<'t> {
    writer: Counting<BufWriter<File>>,
    tracker: &'t Tracker<'t>,
}

impl Sink<'_> {
    fn write_and_flush<T>(
        &mut self,
        write: impl FnOnce(&mut Self) -> Result<T, AudioError>,
    ) -> Result<T, AudioError> {
        let output = write(self)?;
        self.flush().map_err(AudioError::Write)?;
        Ok(output)
    }
}

impl Write for Sink<'_> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.tracker.should_stop() {
            return Err(io::Error::other("the Extraction stopped"));
        }
        self.writer.write(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl EntryWriter<'_> {
    pub(crate) fn write_raw_sound(
        &self,
        destination: &Destination,
        bytes: &[u8],
        source: &SoundArtifactSource<'_>,
        chunk: &mut [u8],
    ) -> Result<SoundOutput, ExtractionError> {
        if source.is_embedded {
            return self.write_raw_bank_member(destination, bytes, source, chunk);
        }
        let input = source.input.map(SoundOriginInput::source);
        self.write_buffered_raw_entry(destination, bytes, &source.entry, input, chunk)
    }

    pub(crate) fn write_buffered_raw_entry(
        &self,
        destination: &Destination,
        bytes: &[u8],
        entry: &SourceEntry<'_>,
        input: Option<SourceInput<'_>>,
        chunk: &mut [u8],
    ) -> Result<SoundOutput, ExtractionError> {
        self.stage_sound(
            destination,
            |sink| {
                let mut measured = OriginDigestWriter::new(sink);
                measured.write_all(bytes).map_err(AudioError::Write)?;
                measured.finish().map_err(AudioError::Write)
            },
            |staged, digest| {
                let outcome = self.publish_observed_raw(
                    staged,
                    digest,
                    input.unwrap_or(SourceInput::Unknown),
                    entry,
                )?;
                Ok(match outcome {
                    StageDisposition::Moved(outcome) => {
                        StageDisposition::Moved(SoundOutput::from_raw_outcome(outcome))
                    }
                    StageDisposition::Cleanup(outcome) => {
                        StageDisposition::Cleanup(SoundOutput::from_raw_outcome(outcome))
                    }
                })
            },
            chunk,
        )?
        .map_err(|error| write_error(destination.file.clone(), io::Error::other(error)))
    }

    fn stage_sound<T>(
        &self,
        destination: &Destination,
        write: impl FnOnce(&mut Sink<'_>) -> Result<T, AudioError>,
        publish: impl FnOnce(
            &StagedOutput<'_>,
            T,
        ) -> Result<StageDisposition<SoundOutput>, ExtractionError>,
        chunk: &mut [u8],
    ) -> Result<Result<SoundOutput, AudioError>, ExtractionError> {
        if destination.existing.is_some() && self.policy == ConflictPolicy::Skip {
            return Ok(Ok(SoundOutput::of(Step::Kept)));
        }
        if self.tracker.should_stop() {
            return Ok(Ok(SoundOutput::of(Step::Cancelled)));
        }
        let (temporary, file, ownership) = self.temporary(&destination.folder)?;
        let mut sink = Sink {
            writer: Counting::new(BufWriter::with_capacity(WRITE_BUFFER_LEN, file)),
            tracker: self.tracker,
        };
        let written = sink.write_and_flush(write);
        let checksum = (sink.writer.len(), Checksum::Crc32c(sink.writer.crc()));
        drop(sink);
        let decided = match written {
            Ok(written) => match self.decide(destination, || checksum, chunk) {
                Ok(None) => {
                    let staged = StagedOutput::new(
                        &temporary,
                        &destination.file,
                        &ownership,
                        self.protection,
                        self.cancel,
                    );
                    match publish(&staged, written) {
                        Ok(StageDisposition::Moved(output)) => return Ok(Ok(output)),
                        Ok(StageDisposition::Cleanup(output)) => Ok(output),
                        Err(error) => Err(error),
                    }
                }
                Ok(Some(outcome)) => Ok(SoundOutput::of(step_of(&outcome))),
                Err(error) => Err(error),
            },
            Err(AudioError::Write(_)) if self.tracker.should_stop() => {
                Ok(SoundOutput::of(Step::Cancelled))
            }
            Err(AudioError::Write(source)) => Err(write_error(destination.file.clone(), source)),
            Err(error) => {
                return match ownership
                    .remove(&temporary, crate::output_file::cancel_flag(self.cancel))
                {
                    Ok(()) => Ok(Err(error)),
                    Err(cleanup) => Err(ExtractionError::CleanupFailed {
                        primary: Some(Box::new(error)),
                        cleanup: Box::new(cleanup),
                    }),
                };
            }
        };
        let decided = match decided {
            Err(error)
                if self.tracker.should_stop()
                    && !crate::guard_cancellation::retained_output(&error) =>
            {
                Ok(SoundOutput::of(Step::Cancelled))
            }
            result => result,
        };
        crate::output_cleanup::extraction(decided.map(Ok), &ownership, &temporary, self.cancel)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use p4kx_audio::AudioError;
    use p4kx_testkit::{ArchiveBuilder, TestEntry};

    use super::Step;
    use crate::tracker::Tracker;
    use crate::write::{Destination, EntryTarget, EntryWriter};
    use crate::{Archive, AudioConversion, CancelToken, ConflictPolicy, TextureConversion};

    #[test]
    fn cancelling_while_a_sound_streams_leaves_neither_file_nor_partial_file() {
        let dir = tempfile::tempdir().expect("temporary folder");
        let path = dir.path().join("Data.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::stored("a.wem", vec![0; 4]))
            .write_to(&path)
            .expect("archive");
        let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key())).expect("open");
        let target = dir.path().join("out");
        fs::create_dir(&target).expect("target");
        let cancel = CancelToken::new();
        let protection =
            crate::ArchiveWriteProtection::new(&path, cancel.flag()).expect("source protection");
        let tracker = Tracker::new(&[], Some(&cancel), None, None);
        let writer = EntryWriter {
            entry_target: EntryTarget::Source,
            protection: Some(&protection),
            cancel: Some(&cancel),
            source: &archive,
            target: &target,
            policy: ConflictPolicy::Overwrite,
            convert: true,
            copy_other: true,
            textures: TextureConversion::default(),
            audio: AudioConversion::Wav,
            verify: false,
            tracker: &tracker,
        };
        let destination = Destination::new(target.clone(), "a.wav".as_ref()).expect("destination");

        let ended = writer
            .stage_sound(
                &destination,
                |sink| {
                    sink.write_all(&[1; 1000]).map_err(AudioError::Write)?;
                    cancel.cancel();
                    sink.write_all(&[2; 1000]).map_err(AudioError::Write)
                },
                |_, ()| panic!("a cancelled sound cannot reach publication"),
                &mut [0; 16],
            )
            .expect("no Extraction error");

        assert!(
            matches!(ended, Ok(output) if output.step == Step::Cancelled),
            "cancelled"
        );
        assert_eq!(
            fs::read_dir(&target).expect("read").count(),
            0,
            "no file and no partial file"
        );
    }
}
