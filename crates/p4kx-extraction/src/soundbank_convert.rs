// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use p4kx_audio::{AudioError, Soundbank};

use crate::buffers::Buffers;
use crate::entry_path::relative_path;
use crate::sound_convert::{Converted, SoundFile};
use crate::sound_job::sound_role;
use crate::source::SourceEntry;
use crate::texture_convert::Step;
use crate::write::{EntryWriter, Outcome};
use crate::{ConversionError, ExtractionError};

/// What unpacking a soundbank got done: how its files ended, whether a WEM was written converted,
/// the first error that stopped a file,
/// the WEMs written unchanged because they could not be converted and the WEMs a cancelled run
/// left unwritten.
#[derive(Default)]
struct Unpacked {
    steps: Vec<Step>,
    converted: bool,
    error: Option<ExtractionError>,
    unconverted: Vec<ConversionError>,
    cancelled: Vec<String>,
}

impl Unpacked {
    fn wrote_wem(&self) -> bool {
        self.steps.iter().skip(1).any(|step| *step == Step::Written)
    }

    fn into_converted(self) -> Converted {
        let outcome = if let Some(error) = self.error {
            Outcome::Failed(error)
        } else if self.converted {
            Outcome::Converted
        } else if self.steps.contains(&Step::Written) {
            Outcome::Written
        } else {
            Outcome::Kept
        };
        Converted {
            outcome,
            unconverted: self.unconverted,
            cancelled: self.cancelled,
        }
    }
}

impl EntryWriter<'_> {
    /// Writes the Soundbank `entry` unchanged and unpacks its WEMs into the folder of its path
    /// without the extension, each named by its ID and converted as the run's audio option says;
    /// a Soundbank that cannot be read is written unchanged and returned.
    ///
    /// The folder is created only when a WEM is written into it. A run cancelled after the
    /// Soundbank itself was written lists each WEM not written as cancelled, as `<path> WEM <ID>`.
    pub(crate) fn try_write_bank(
        &self,
        entry: &SourceEntry<'_>,
        buffers: &mut Buffers,
        credited: &AtomicU64,
    ) -> Result<Converted, ExtractionError> {
        if self.tracker.should_stop() {
            return Ok(Converted::of(Outcome::Cancelled));
        }
        let disk_name = self.disk_name(entry);
        let stem = sound_role(&disk_name)
            .map(|(_, stem)| stem)
            .ok_or(ExtractionError::UnsafePath)?;
        let wem_folder = relative_path(stem)?;
        let destination = self.destination(entry)?;
        if !self.read_sound(entry, buffers, credited)? {
            return Ok(Converted::of(Outcome::Cancelled));
        }
        let Buffers { chunk, data, .. } = buffers;
        let bytes = data.as_slice();
        let raw = self
            .write_converted(
                &destination,
                |sink| sink.write_all(bytes).map_err(AudioError::Write),
                chunk,
            )?
            .unwrap_or(Step::Cancelled);
        if raw == Step::Cancelled {
            return Ok(Converted::of(Outcome::Cancelled));
        }
        let mut unpacked = Unpacked {
            steps: vec![raw],
            ..Unpacked::default()
        };
        match Soundbank::parse(bytes) {
            Ok(bank) => {
                if bank.wems().next().is_some() {
                    let existed = self.target.join(&wem_folder).symlink_metadata().is_ok();
                    let folder = self.folder(Some(&wem_folder))?;
                    self.unpack(
                        entry,
                        &bank,
                        (folder.as_path(), chunk),
                        credited,
                        &mut unpacked,
                    );
                    if !existed && !unpacked.wrote_wem() {
                        let _ignored = fs::remove_dir(&folder);
                    }
                }
                Ok(unpacked.into_converted())
            }
            Err(source) => {
                let error = ConversionError::Soundbank {
                    bank: entry.path().to_owned(),
                    source,
                };
                let mut converted = unpacked.into_converted();
                converted.unconverted.push(error);
                Ok(converted)
            }
        }
    }

    /// Writes every WEM of `bank` into `folder` in ID order, counting the rest of the soundbank's
    /// size as progress in equal parts; stops once a file fails to be written, and names the WEMs
    /// left unwritten once the run is cancelled.
    fn unpack(
        &self,
        entry: &SourceEntry<'_>,
        bank: &Soundbank<&[u8]>,
        (folder, chunk): (&Path, &mut [u8]),
        credited: &AtomicU64,
        unpacked: &mut Unpacked,
    ) {
        let rest = entry
            .uncompressed_size()
            .saturating_sub(credited.load(Ordering::Relaxed));
        let share = rest
            .checked_div(bank.wems().len() as u64)
            .unwrap_or_default();
        let mut stem = String::new();
        let mut name = String::new();
        for (position, wem) in bank.wems().enumerate() {
            stem.clear();
            name.clear();
            let _infallible = write!(stem, "{}", wem.id());
            let _infallible = write!(name, "{} WEM {}", entry.path(), wem.id());
            let sound = SoundFile {
                folder,
                stem: &stem,
                name: &name,
                prefetch: wem.is_prefetch(),
            };
            match self.write_sound(wem.bytes(), &sound, chunk) {
                Ok(end) => {
                    unpacked.converted |= end.converted && end.step == Step::Written;
                    unpacked.steps.push(end.step);
                    unpacked.unconverted.extend(end.unconverted);
                    if end.step == Step::Cancelled {
                        unpacked.cancelled = bank
                            .wems()
                            .skip(position)
                            .map(|left| format!("{} WEM {}", entry.path(), left.id()))
                            .collect();
                        return;
                    }
                }
                Err(error) => {
                    unpacked.error = Some(error);
                    return;
                }
            }
            credited.fetch_add(share, Ordering::Relaxed);
            self.tracker.wrote(share);
        }
    }
}
