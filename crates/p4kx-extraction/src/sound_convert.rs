// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicU64;

use p4kx_audio::{AudioError, Codec, MediaClass, Wem};

use crate::buffers::Buffers;
use crate::entry_path::relative_path;
use crate::extract::Ended;
use crate::sound_job::sound_role;
use crate::source::SourceEntry;
use crate::texture_convert::Step;
use crate::texture_job::Indexed;
use crate::write::{Destination, EntryWriter, Outcome};
use crate::{AudioConversion, ConflictPolicy, ConversionError, EntryFailure, ExtractionError};

/// One sound to write: the folder it goes to, its file name without extension, the name its
/// errors carry and whether it is only the prefetched start of a streamed WEM.
pub(crate) struct SoundFile<'a> {
    pub(crate) folder: &'a Path,
    pub(crate) stem: &'a str,
    pub(crate) name: &'a str,
    pub(crate) prefetch: bool,
}

/// How writing one sound ended: its step, whether it was converted rather than written
/// unchanged, and why it was written unchanged when it could not be converted.
pub(crate) struct SoundEnd {
    pub(crate) step: Step,
    pub(crate) converted: bool,
    pub(crate) unconverted: Option<ConversionError>,
}

/// The forms a sound is tried in, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    Ogg,
    Wav,
    Raw,
    Midi,
}

impl Form {
    const fn first(audio: AudioConversion) -> Self {
        match audio {
            AudioConversion::Ogg => Self::Ogg,
            AudioConversion::Wav => Self::Wav,
            AudioConversion::Raw => Self::Raw,
        }
    }

    const fn extension(self) -> &'static str {
        match self {
            Self::Ogg => "ogg",
            Self::Wav => "wav",
            Self::Raw => "wem",
            Self::Midi => "mid",
        }
    }
}

/// How converting one sound or Soundbank Entry ended, what it could not convert and the sounds of
/// a Soundbank that the cancelled run left unwritten.
pub(crate) struct Converted {
    pub(crate) outcome: Outcome,
    pub(crate) unconverted: Vec<ConversionError>,
    pub(crate) cancelled: Vec<String>,
}

impl Converted {
    pub(crate) const fn of(outcome: Outcome) -> Self {
        Self {
            outcome,
            unconverted: Vec::new(),
            cancelled: Vec::new(),
        }
    }
}

impl EntryWriter<'_> {
    /// Converts the sound or Soundbank `entry` through `convert` and reports its end; an Entry
    /// larger than `limit` is written unchanged and reported as not converted.
    ///
    /// `convert` counts the bytes it reports as progress in its `AtomicU64`.
    pub(crate) fn convert_sound_entry(
        &self,
        (index, entry): Indexed<'_>,
        buffers: &mut Buffers,
        limit: u64,
        convert: impl FnOnce(
            &Self,
            &SourceEntry<'_>,
            &mut Buffers,
            &AtomicU64,
        ) -> Result<Converted, ExtractionError>,
    ) -> Ended {
        let path = entry.path().to_owned();
        if entry.uncompressed_size() > limit {
            let outcome = self.write_raw(entry, buffers);
            let error = ExtractionError::TooLargeToConvert {
                size: entry.uncompressed_size(),
                limit,
            };
            return Ended::Sound {
                entry: (index, path.clone(), outcome),
                unconverted: vec![EntryFailure { path, error }],
                cancelled: Vec::new(),
            };
        }
        let credited = AtomicU64::new(0);
        let done = match convert(self, entry, buffers, &credited) {
            Ok(done) => done,
            Err(error) if crate::guard_cancellation::retained_output(&error) => {
                Converted::of(Outcome::Failed(error))
            }
            Err(_) if self.tracker.archive_file_changed() => Converted::of(Outcome::Cancelled),
            Err(error) => Converted::of(Outcome::Failed(error)),
        };
        buffers.trim();
        self.tracker.ended(entry, credited.into_inner());
        let unconverted = done
            .unconverted
            .into_iter()
            .map(|error| EntryFailure {
                path: path.clone(),
                error: ExtractionError::Sound(Box::new(error)),
            })
            .collect();
        Ended::Sound {
            entry: (index, path, done.outcome),
            unconverted,
            cancelled: done.cancelled,
        }
    }

    /// Reads the whole Entry into `buffers.data`, counting half of it as progress in `credited`;
    /// returns `false` once the run is to stop.
    pub(crate) fn read_sound(
        &self,
        entry: &SourceEntry<'_>,
        buffers: &mut Buffers,
        credited: &AtomicU64,
    ) -> Result<bool, ExtractionError> {
        let len = usize::try_from(entry.uncompressed_size()).unwrap_or(usize::MAX);
        let mut reader = self.open_entry(entry)?;
        let Buffers { chunk, data, .. } = buffers;
        data.clear();
        self.read_all(&mut reader, data, len, chunk, credited)
    }

    /// Converts the WEM sound `entry` as the run's audio option says; a sound that cannot be
    /// converted is written unchanged and returned. Under [`ConflictPolicy::Skip`] an existing
    /// output is kept without reading the sound, except for Ogg, whose output depends on the
    /// codec only the sound tells.
    pub(crate) fn try_write_sound(
        &self,
        entry: &SourceEntry<'_>,
        buffers: &mut Buffers,
        credited: &AtomicU64,
    ) -> Result<Converted, ExtractionError> {
        if self.tracker.should_stop() {
            return Ok(Converted::of(Outcome::Cancelled));
        }
        let disk_name = self.disk_name(entry);
        let relative = relative_path(&disk_name)?;
        let stem = sound_role(&disk_name)
            .and_then(|(_, stem)| stem.rsplit(['\\', '/']).next())
            .ok_or(ExtractionError::UnsafePath)?;
        let folder = self.folder(relative.parent())?;
        let sound = SoundFile {
            folder: &folder,
            stem,
            name: entry.path(),
            prefetch: false,
        };
        if self.policy == ConflictPolicy::Skip
            && self.audio != AudioConversion::Ogg
            && sound_destination(&sound, Form::first(self.audio))?
                .existing
                .is_some()
        {
            return Ok(Converted::of(Outcome::Kept));
        }
        if !self.read_sound(entry, buffers, credited)? {
            return Ok(Converted::of(Outcome::Cancelled));
        }
        let end = self.write_sound(&buffers.data, &sound, &mut buffers.chunk)?;
        let outcome = match end.step {
            Step::Written if end.converted => Outcome::Converted,
            Step::Written => Outcome::Written,
            Step::Kept => Outcome::Kept,
            Step::Cancelled => Outcome::Cancelled,
        };
        Ok(Converted {
            unconverted: end.unconverted.into_iter().collect(),
            ..Converted::of(outcome)
        })
    }

    /// Writes the WEM `bytes` into its folder: as Ogg where the audio option asks for it and the
    /// codec has an Ogg form, else as WAV unless the option keeps sounds unchanged; a sound that
    /// cannot be converted, plug-in media and a prefetched start are written unchanged as `.wem` and MIDI as `.mid`,
    /// and only a sound that cannot be converted carries its error.
    pub(crate) fn write_sound(
        &self,
        bytes: &[u8],
        sound: &SoundFile<'_>,
        chunk: &mut [u8],
    ) -> Result<SoundEnd, ExtractionError> {
        let first = Form::first(self.audio);
        let parsed = if first == Form::Raw || sound.prefetch {
            Ok(None)
        } else {
            match Wem::parse(bytes) {
                Ok(wem) => Ok(Some(wem)),
                Err(AudioError::PluginMedia | AudioError::MidiMedia) => Ok(None),
                Err(error) => Err(error),
            }
        };
        let failure = match parsed {
            Ok(Some(wem)) => match self.convert_sound(&wem, sound, first, chunk)? {
                Ok(step) => {
                    return Ok(SoundEnd {
                        step,
                        converted: true,
                        unconverted: None,
                    });
                }
                Err(error) => Some(error),
            },
            Ok(None) => None,
            Err(error) => Some(error),
        };
        let unchanged = if MediaClass::of(bytes) == MediaClass::Midi {
            Form::Midi
        } else {
            Form::Raw
        };
        let destination = sound_destination(sound, unchanged)?;
        let step = self
            .write_converted(
                &destination,
                |sink| sink.write_all(bytes).map_err(AudioError::Write),
                chunk,
            )?
            .unwrap_or(Step::Cancelled);
        Ok(SoundEnd {
            step,
            converted: false,
            unconverted: failure.filter(|_| step != Step::Cancelled).map(|source| {
                ConversionError::Audio {
                    sound: sound.name.to_owned(),
                    source,
                }
            }),
        })
    }

    /// Writes `wem` in the form `first`, as WAV instead of Ogg for a codec without an Ogg form;
    /// returns the error of a sound that cannot be converted.
    fn convert_sound(
        &self,
        wem: &Wem<&[u8]>,
        sound: &SoundFile<'_>,
        first: Form,
        chunk: &mut [u8],
    ) -> Result<Result<Step, AudioError>, ExtractionError> {
        let mut form = match first {
            Form::Ogg if !matches!(wem.codec(), Codec::Vorbis | Codec::Opus) => Form::Wav,
            form => form,
        };
        loop {
            let destination = sound_destination(sound, form)?;
            let written = self.write_converted(
                &destination,
                |sink| match form {
                    Form::Ogg => wem.write_ogg(sink).map(drop),
                    Form::Wav | Form::Raw | Form::Midi => wem.write_wav(sink).map(drop),
                },
                chunk,
            )?;
            match written {
                Err(AudioError::UnsupportedOggCodec(_)) if form == Form::Ogg => form = Form::Wav,
                written => return Ok(written),
            }
        }
    }
}

fn sound_destination(sound: &SoundFile<'_>, form: Form) -> Result<Destination, ExtractionError> {
    let name = format!("{}.{}", sound.stem, form.extension());
    Destination::new(sound.folder.to_path_buf(), name.as_ref())
}
