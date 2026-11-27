// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::ArchiveWatch;

/// What an Extraction does with a file that already exists in the target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ConflictPolicy {
    /// Keeps the existing file and counts the Entry as skipped.
    #[default]
    Skip,
    /// Replaces the existing file.
    Overwrite,
    /// Replaces the existing file only when its size or CRC-32C differs from the Entry's.
    IfChanged,
}

/// A shared flag that asks a running Extraction to stop.
///
/// Clones share the flag, so one clone can be handed to a signal handler or another thread.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// Creates a token that is not cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks every Extraction holding this token to stop.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Returns whether [`cancel`](Self::cancel) was called.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Returns the shared cancellation flag for synchronous guarded operations.
    #[must_use]
    pub fn flag(&self) -> &AtomicBool {
        &self.0
    }
}

/// How far an Extraction has come.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progress {
    /// Entries that ended: written, skipped, failed or cancelled.
    pub entries_done: usize,
    /// Entries of the run.
    pub entries_total: usize,
    /// Uncompressed bytes written so far, plus the full size of every Entry that ended.
    pub bytes_done: u64,
    /// Uncompressed size of every Entry of the run.
    pub bytes_total: u64,
}

/// The settings of one Extraction.
#[derive(Clone, Copy, Default)]
pub struct ExtractOptions<'a> {
    /// Immutable source-origin evidence shared by an Archive-backed output job.
    pub protection: Option<&'a p4kx_guard::ArchiveWriteProtection>,
    /// What to do with files that already exist.
    pub policy: ConflictPolicy,
    /// Converts `CryXml` Entries to UTF-8 XML, split DDS textures as `textures` says, WEM sounds
    /// and soundbanks as `audio` says and the DataForge Entry to Record XML files; text XML and
    /// every other Entry stay unchanged.
    pub convert: bool,
    /// What a Conversion writes for a DDS texture; ignored while `convert` is off.
    pub textures: TextureConversion,
    /// What a Conversion writes for a WEM sound and for each WEM a soundbank embeds; ignored
    /// while `convert` is off.
    pub audio: AudioConversion,
    /// Checks the data of each Entry against the checksum of the directory while it is written
    /// and reports a mismatch as a failure of that Entry, which is then not written. Costs one
    /// checksum over the decoded bytes; a texture header that a PNG or EXR export
    /// parses only in part is read to its end first. An Entry skipped by the Conflict policy is not
    /// read and so not verified.
    pub verify: bool,
    /// The most threads the whole run uses, sound workers and the work nested in them included,
    /// on a pool of its own, capped at the logical processors; `None` runs on the current rayon
    /// pool.
    pub threads: Option<NonZeroUsize>,
    /// Stops the run once cancelled.
    pub cancel: Option<&'a CancelToken>,
    /// Called from any worker thread whenever an Entry ends or a chunk of data is written.
    pub progress: Option<&'a (dyn Fn(Progress) + Sync)>,
    /// Checked before the run, periodically during it and after it; a change stops the run.
    pub watch: Option<&'a ArchiveWatch>,
}

impl fmt::Debug for ExtractOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtractOptions")
            .field("policy", &self.policy)
            .field("convert", &self.convert)
            .field("textures", &self.textures)
            .field("audio", &self.audio)
            .field("verify", &self.verify)
            .field("threads", &self.threads)
            .field("cancel", &self.cancel)
            .field("progress", &self.progress.is_some())
            .field("watch", &self.watch)
            .finish()
    }
}

/// What a Conversion writes for a WEM sound `<name>.wem` and for each WEM a soundbank
/// `<name>.bnk` embeds.
///
/// A soundbank is written unchanged and unpacked into the folder `<name>` next to it, each WEM
/// named by its ID. A sound whose codec cannot be decoded, plug-in media and the prefetched start
/// of a streamed WEM are written unchanged as `.wem`; only a sound that cannot be decoded is
/// reported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AudioConversion {
    /// Rebuilds Wwise Vorbis as Ogg Vorbis and Opus as Ogg Opus, `<name>.ogg`; every other codec
    /// is decoded to WAV, `<name>.wav`.
    #[default]
    Ogg,
    /// Decodes every sound to a 16-bit PCM WAV, `<name>.wav`.
    Wav,
    /// Writes every sound unchanged, `<name>.wem`.
    Raw,
}

/// What a Conversion writes for a DDS texture: the header Entry `<name>.dds` and its parts
/// `<name>.dds.1` … `<name>.dds.N`.
///
/// The outputs combine. The merged DDS takes the header's path, the PNG is `<name>.png` next to
/// it, with an alpha tail `<name>.dds.a` in its alpha channel; without a PNG, the merge writes
/// the alpha tail merged under its own path. With neither the merge nor the PNG chosen, the
/// texture's Entries are written unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureConversion {
    /// Writes the header and its parts merged into one standard DDS file.
    pub merge: bool,
    /// Writes one mip as an 8-bit RGBA PNG, a high-range texture as OpenEXR: the given level, 0
    /// being the largest, or the smallest mip when the texture has fewer levels; `None` writes
    /// no image.
    pub png: Option<u32>,
    /// Also writes the Entries unchanged; with the merge, the merged DDS replaces the raw header.
    pub keep_raw: bool,
}

impl TextureConversion {
    /// Returns whether a texture is converted at all rather than written unchanged.
    #[must_use]
    pub const fn converts(&self) -> bool {
        self.merge || self.png.is_some()
    }
}

impl Default for TextureConversion {
    /// Merges the texture and exports its largest mip as PNG, without the raw parts.
    fn default() -> Self {
        Self {
            merge: true,
            png: Some(0),
            keep_raw: false,
        }
    }
}
