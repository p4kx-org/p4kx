// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use p4kx_archive::Checksum;
use p4kx_audio::AudioError;

use crate::counting::Counting;
use crate::source::SourceEntry;
use crate::texture_convert::Step;
use crate::texture_outputs::step_of;
use crate::tracker::Tracker;
use crate::write::{Chunk, Destination, EntryWriter, Outcome, write_error};
use crate::{ConflictPolicy, ConvertedSoundArtifact, ExtractionError, OwnedFileToken, SourceInput};

#[path = "sound_linear_output.rs"]
mod linear;

pub(crate) struct LooseSound<'a> {
    entry: SourceEntry<'a>,
    input: Option<SourceInput<'a>>,
}

impl<'a> LooseSound<'a> {
    pub(crate) const fn new(entry: SourceEntry<'a>, input: Option<SourceInput<'a>>) -> Self {
        Self { entry, input }
    }
}

pub(crate) struct SoundOutput {
    pub(crate) step: Step,
    pub(crate) artifact: Option<ConvertedSoundArtifact>,
    pub(crate) receipt_failure: Option<ExtractionError>,
}

impl SoundOutput {
    pub(crate) const fn of(step: Step) -> Self {
        Self {
            step,
            artifact: None,
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
    /// Writes the file of `destination` through `write` under the Conflict policy: into a
    /// temporary file first, compared with an existing file by length and CRC-32C under
    /// [`ConflictPolicy::IfChanged`], then put in place.
    ///
    /// Returns how the file ended, or the error `write` returned other than a failed write, which
    /// leaves nothing behind; `chunk` is the buffer for reading an existing file.
    pub(crate) fn write_converted(
        &self,
        destination: &Destination,
        write: impl FnOnce(&mut Sink<'_>) -> Result<(), AudioError>,
        chunk: &mut [u8],
    ) -> Result<Result<Step, AudioError>, ExtractionError> {
        self.stage_converted(
            destination,
            write,
            |temporary, ownership, ()| {
                self.put_in_place(
                    temporary,
                    ownership,
                    &destination.file,
                    Ok(Chunk::End),
                    Outcome::Converted,
                )
                .map(|outcome| SoundOutput::of(step_of(&outcome)))
            },
            chunk,
        )
        .map(|result| result.map(|output| output.step))
    }

    fn stage_converted<T>(
        &self,
        destination: &Destination,
        write: impl FnOnce(&mut Sink<'_>) -> Result<T, AudioError>,
        publish: impl FnOnce(&Path, &OwnedFileToken, T) -> Result<SoundOutput, ExtractionError>,
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
        let written = write(&mut sink).and_then(|written| {
            sink.writer.flush().map_err(AudioError::Write)?;
            Ok(written)
        });
        let checksum = (sink.writer.len(), Checksum::Crc32c(sink.writer.crc()));
        drop(sink);
        let decided = match written {
            Ok(written) => match self.decide(destination, || checksum, chunk) {
                Ok(None) => return publish(&temporary, &ownership, written).map(Ok),
                Ok(Some(outcome)) => Ok(step_of(&outcome)),
                Err(error) => Err(error),
            },
            Err(AudioError::Write(_)) if self.tracker.should_stop() => Ok(Step::Cancelled),
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
                Ok(Step::Cancelled)
            }
            result => result,
        };
        crate::output_cleanup::extraction(
            decided.map(|step| Ok(SoundOutput::of(step))),
            &ownership,
            &temporary,
            self.cancel,
        )
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
            .write_converted(
                &destination,
                |sink| {
                    sink.write_all(&[1; 1000]).map_err(AudioError::Write)?;
                    cancel.cancel();
                    sink.write_all(&[2; 1000]).map_err(AudioError::Write)
                },
                &mut [0; 16],
            )
            .expect("no Extraction error");

        assert!(matches!(ended, Ok(Step::Cancelled)), "cancelled");
        assert_eq!(
            fs::read_dir(&target).expect("read").count(),
            0,
            "no file and no partial file"
        );
    }
}
