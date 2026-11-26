// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use p4kx_extraction::{Codec, EmbeddedWem, EntryOrigin, ObservedEntry, Soundbank, Wem};

use super::audio_worker::{Source, Worker};
use super::bank_cache::BankCache;
use super::wem_header_provenance::{WemHeaderEvidenceError, WemHeaderProvenance};
use crate::EntryIndex;
use crate::audio::{Pcm, Player, Undecoded};

static SERIAL: AtomicU64 = AtomicU64::new(0);

/// Returns whether `path` names a Wwise WEM sound the Audio tab plays.
#[must_use]
pub fn is_wem_path(path: &str) -> bool {
    let extension = path
        .len()
        .checked_sub(4)
        .and_then(|start| path.get(start..));
    extension.is_some_and(|extension| extension.eq_ignore_ascii_case(".wem"))
}

/// What the header of a WEM says about the sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundInfo {
    /// The codec.
    pub codec: Codec,
    /// The number of channels.
    pub channels: u16,
    /// The sample rate in Hz.
    pub sample_rate: u32,
    /// The playing time, when the header states it or the codec allows computing it.
    pub duration: Option<Duration>,
}

impl SoundInfo {
    /// Reads the facts from the header of `wem`.
    pub(super) fn of<B: AsRef<[u8]>>(wem: &Wem<B>) -> Self {
        Self {
            codec: wem.codec(),
            channels: wem.channels(),
            sample_rate: wem.sample_rate(),
            duration: wem.duration(),
        }
    }
}

/// Where the audio preview of a WEM stands.
#[derive(Debug, Clone, Copy)]
pub enum AudioStatus<'a> {
    /// The WEM is being read.
    Opening,
    /// The WEM is larger than [`PREVIEW_SIZE_LIMIT`](crate::PREVIEW_SIZE_LIMIT) and was not read.
    TooLarge,
    /// The WEM could not be read, for the reason given, or reading stopped without a result.
    Failed(Option<&'a str>),
    /// The Archive changed on disk, so the WEM was not read or its sound was dropped.
    ArchiveChanged,
    /// The header was read; the sound stands as given.
    Opened(&'a SoundInfo, SoundStatus<'a>),
}

/// Where the decoded sound of an opened WEM stands.
#[derive(Debug, Clone, Copy)]
pub enum SoundStatus<'a> {
    /// The sound is being decoded.
    Decoding,
    /// The preview has no decoder for the codec.
    Unsupported(Codec),
    /// The decoded sound would exceed [`MAX_PREVIEW_PCM`](crate::MAX_PREVIEW_PCM).
    TooLong,
    /// The sound could not be decoded, for the reason given, or decoding stopped without a
    /// result.
    Failed(Option<&'a str>),
    /// The sound was decoded and plays through this player.
    Ready(&'a Player),
}

#[derive(Debug)]
pub(super) struct SoundHeader {
    info: SoundInfo,
    provenance: Option<Result<Box<WemHeaderProvenance>, WemHeaderEvidenceError>>,
}

impl SoundHeader {
    pub(super) fn observed<B: AsRef<[u8]>>(
        wem: &Wem<B>,
        source: Option<ObservedEntry<'_>>,
    ) -> Self {
        Self {
            info: SoundInfo::of(wem),
            provenance: Some(WemHeaderProvenance::capture(wem, source).map(Box::new)),
        }
    }

    pub(super) fn observed_embedded<B: AsRef<[u8]>>(
        wem: &Wem<B>,
        embedded: &EmbeddedWem<'_>,
        source: Option<&EntryOrigin>,
    ) -> Self {
        let provenance = WemHeaderProvenance::capture_embedded(wem, embedded, source);
        Self {
            info: SoundInfo::of(wem),
            provenance: Some(provenance.map(Box::new)),
        }
    }
}

#[derive(Debug)]
pub(super) enum Event {
    TooLarge,
    Failed(Option<String>),
    ArchiveChanged,
    Opened(SoundHeader),
    Decoded(Result<Pcm, Undecoded>),
}

#[derive(Debug)]
enum Sound {
    Decoding,
    Unsupported(Codec),
    TooLong,
    Failed(Option<String>),
    Ready(Player),
}

#[derive(Debug)]
enum State {
    Opening,
    TooLarge,
    Failed(Option<String>),
    ArchiveChanged,
    Opened(SoundHeader, Sound),
}

/// The audio preview of a WEM: read and decoded to PCM on a background thread, then played
/// through a [`Player`]. Dropping it stops decoding and playback.
#[derive(Debug)]
pub struct AudioPreview {
    serial: u64,
    dropped: Arc<AtomicBool>,
    events: Receiver<Event>,
    watched_index: Option<Arc<EntryIndex>>,
    state: State,
}

impl AudioPreview {
    /// Starts reading and decoding the WEM Entry `id` of `index` on a background thread,
    /// calling `wake` from it whenever a result is ready.
    pub(crate) fn start(
        index: &Arc<EntryIndex>,
        id: u32,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        Self::spawn(Source::Entry(Arc::clone(index), id), wake)
    }

    /// Starts decoding the WEM `id` embedded in `bank` on a background thread, calling `wake`
    /// from it whenever a result is ready.
    pub(crate) fn start_embedded(
        bank: &Arc<Soundbank<Vec<u8>>>,
        id: u32,
        original: Option<&Arc<EntryOrigin>>,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        Self::spawn(
            Source::Embedded {
                bank: Arc::clone(bank),
                id,
                original: original.cloned(),
            },
            wake,
        )
    }

    /// Starts reading the soundbank Entry `bank` of `index`, unless `cache` keeps it, and decoding
    /// the WEM `id` it embeds on a background thread, calling `wake` from it whenever a result is
    /// ready. A soundbank that was read is kept in `cache`.
    pub(crate) fn start_in_bank(
        index: &Arc<EntryIndex>,
        (bank, id): (u32, u32),
        cache: &BankCache,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let source = Source::InBank(Arc::clone(index), (bank, id), cache.clone());
        Self::spawn(source, wake)
    }

    fn spawn(source: Source, wake: impl Fn() + Send + 'static) -> Self {
        let (sender, events) = mpsc::channel();
        let dropped = Arc::new(AtomicBool::new(false));
        let watched_index = match &source {
            Source::Entry(index, _) | Source::InBank(index, _, _) => Some(Arc::clone(index)),
            Source::Embedded { .. } => None,
        };
        let worker = Worker::new(source, sender, Arc::clone(&dropped));
        let spawned = thread::Builder::new()
            .name("audio-preview".to_owned())
            .spawn(move || worker.run(&wake));
        let state = match spawned {
            Ok(_handle) => State::Opening,
            Err(_ignored) => State::Failed(None),
        };
        Self {
            serial: SERIAL.fetch_add(1, Ordering::Relaxed),
            dropped,
            events,
            watched_index,
            state,
        }
    }

    /// Returns a number no other audio preview of this process has.
    #[must_use]
    pub const fn serial(&self) -> u64 {
        self.serial
    }

    /// Returns where the preview stands.
    #[must_use]
    pub fn status(&self) -> AudioStatus<'_> {
        if self.archive_changed() {
            return AudioStatus::ArchiveChanged;
        }
        match &self.state {
            State::Opening => AudioStatus::Opening,
            State::TooLarge => AudioStatus::TooLarge,
            State::Failed(reason) => AudioStatus::Failed(reason.as_deref()),
            State::ArchiveChanged => AudioStatus::ArchiveChanged,
            State::Opened(header, sound) => AudioStatus::Opened(&header.info, sound_status(sound)),
        }
    }

    /// Returns the original contributions to the facts of an opened WEM.
    #[must_use]
    pub fn header_provenance(
        &self,
    ) -> Option<Result<&WemHeaderProvenance, &WemHeaderEvidenceError>> {
        if self.archive_changed() {
            return None;
        }
        match &self.state {
            State::Opened(header, _) => header.provenance.as_ref().map(Result::as_deref),
            _ => None,
        }
    }

    /// Returns whether the WEM is being read or decoded.
    #[must_use]
    pub const fn is_busy(&self) -> bool {
        matches!(
            self.state,
            State::Opening | State::Opened(_, Sound::Decoding)
        )
    }

    /// Returns the player once the sound was decoded.
    #[must_use]
    pub const fn player(&self) -> Option<&Player> {
        match &self.state {
            State::Opened(_, Sound::Ready(player)) => Some(player),
            _ => None,
        }
    }

    /// Returns the player for controlling it, once the sound was decoded.
    pub const fn player_mut(&mut self) -> Option<&mut Player> {
        match &mut self.state {
            State::Opened(_, Sound::Ready(player)) => Some(player),
            _ => None,
        }
    }

    /// Takes the results of the background thread and a failure of the audio output; returns
    /// whether one arrived.
    pub(crate) fn poll(&mut self) -> bool {
        if self.archive_changed() {
            let changed = !matches!(self.state, State::ArchiveChanged);
            self.dropped.store(true, Ordering::Release);
            self.state = State::ArchiveChanged;
            return changed;
        }
        let mut changed = self.player_mut().is_some_and(Player::poll);
        loop {
            match self.events.try_recv() {
                Ok(event) => {
                    self.apply(event);
                    changed = true;
                }
                Err(TryRecvError::Empty) => return changed,
                Err(TryRecvError::Disconnected) => return self.lost() || changed,
            }
        }
    }

    fn archive_changed(&self) -> bool {
        self.watched_index
            .as_ref()
            .is_some_and(|index| index.watch().is_changed())
    }

    fn apply(&mut self, event: Event) {
        self.state = match event {
            Event::TooLarge => State::TooLarge,
            Event::Failed(reason) => State::Failed(reason),
            Event::ArchiveChanged => State::ArchiveChanged,
            Event::Opened(header) => State::Opened(header, Sound::Decoding),
            Event::Decoded(decoded) => {
                let State::Opened(_, sound @ Sound::Decoding) = &mut self.state else {
                    return;
                };
                *sound = match decoded {
                    Ok(pcm) => Sound::Ready(Player::new(pcm, 1.0)),
                    Err(Undecoded::Unsupported(codec)) => Sound::Unsupported(codec),
                    Err(Undecoded::TooLong) => Sound::TooLong,
                    Err(Undecoded::Cancelled) => Sound::Failed(None),
                    Err(Undecoded::Failed(reason)) => Sound::Failed(Some(reason)),
                };
                return;
            }
        };
    }

    fn lost(&mut self) -> bool {
        match &mut self.state {
            State::Opening => self.state = State::Failed(None),
            State::Opened(_, sound @ Sound::Decoding) => *sound = Sound::Failed(None),
            _ => return false,
        }
        true
    }
}

impl Drop for AudioPreview {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

fn sound_status(sound: &Sound) -> SoundStatus<'_> {
    match sound {
        Sound::Decoding => SoundStatus::Decoding,
        Sound::Unsupported(codec) => SoundStatus::Unsupported(*codec),
        Sound::TooLong => SoundStatus::TooLong,
        Sound::Failed(reason) => SoundStatus::Failed(reason.as_deref()),
        Sound::Ready(player) => SoundStatus::Ready(player),
    }
}
