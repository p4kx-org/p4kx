// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use p4kx_extraction::{EntryOrigin, Soundbank, SourceEvidenceError};

use super::audio::AudioPreview;
use super::bank_sound::{BankSound, BankWemRow};
use super::soundbank_provenance::{BankSoundProvenance, SoundbankSummaryProvenance};
use super::soundbank_worker::Worker;
use super::wem_header_provenance::WemHeaderEvidenceError;
use crate::EntryIndex;

static SERIAL: AtomicU64 = AtomicU64::new(0);

/// Returns whether `path` names a Wwise soundbank the Audio tab lists the sounds of.
#[must_use]
pub fn is_bank_path(path: &str) -> bool {
    let extension = path
        .len()
        .checked_sub(4)
        .and_then(|start| path.get(start..));
    extension.is_some_and(|extension| extension.eq_ignore_ascii_case(".bnk"))
}

/// Where the preview of a soundbank stands.
#[derive(Debug, Clone, Copy)]
pub enum SoundbankStatus<'a> {
    /// The soundbank is being read.
    Opening,
    /// The soundbank is larger than [`PREVIEW_SIZE_LIMIT`](crate::PREVIEW_SIZE_LIMIT) and was
    /// not read.
    TooLarge,
    /// The soundbank could not be read or parsed, for the reason given, or reading stopped
    /// without a result.
    Failed(Option<&'a str>),
    /// The Archive changed on disk, so the soundbank was not read or its sounds were dropped.
    ArchiveChanged,
    /// The soundbank was parsed; these are the WEMs it embeds in ascending ID order.
    Opened(&'a [BankSound]),
}

#[derive(Debug)]
pub(super) struct Opened {
    bank: Arc<Soundbank<Vec<u8>>>,
    original: Option<Arc<EntryOrigin>>,
    sounds: Vec<BankSound>,
    evidence: Vec<BankWemRow>,
    summary: Result<SoundbankSummaryProvenance, SourceEvidenceError>,
}

impl Opened {
    pub(super) const fn new(
        bank: Arc<Soundbank<Vec<u8>>>,
        original: Option<Arc<EntryOrigin>>,
        sounds: Vec<BankSound>,
        evidence: Vec<BankWemRow>,
        summary: Result<SoundbankSummaryProvenance, SourceEvidenceError>,
    ) -> Self {
        Self {
            bank,
            original,
            sounds,
            evidence,
            summary,
        }
    }

    fn position(&self, id: u32) -> Option<usize> {
        self.sounds.binary_search_by_key(&id, BankSound::id).ok()
    }
}

#[derive(Debug)]
pub(super) enum Event {
    TooLarge,
    Failed(Option<String>),
    ArchiveChanged,
    Opened(Box<Opened>),
}

#[derive(Debug)]
enum State {
    Opening,
    TooLarge,
    Failed(Option<String>),
    ArchiveChanged,
    Opened(Box<Opened>),
}

#[derive(Debug)]
struct Selected {
    id: u32,
    audio: Option<AudioPreview>,
}

/// The preview of a Wwise soundbank, read and parsed on a background thread.
///
/// It lists the WEMs the soundbank embeds, one of which can be selected and played through an
/// [`AudioPreview`]. Dropping it stops reading and playback.
#[derive(Debug)]
pub struct SoundbankPreview {
    serial: u64,
    dropped: Arc<AtomicBool>,
    events: Receiver<Event>,
    index: Arc<EntryIndex>,
    state: State,
    selected: Option<Selected>,
}

impl SoundbankPreview {
    /// Starts reading the soundbank Entry `id` of `index` on a background thread, calling `wake`
    /// from it once the sounds are listed.
    pub(crate) fn start(
        index: &Arc<EntryIndex>,
        id: u32,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let (sender, events) = mpsc::channel();
        let dropped = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            index: Arc::clone(index),
            events: sender,
            dropped: Arc::clone(&dropped),
        };
        let spawned = thread::Builder::new()
            .name("soundbank-preview".to_owned())
            .spawn(move || worker.run(id, &wake));
        let state = match spawned {
            Ok(_handle) => State::Opening,
            Err(_ignored) => State::Failed(None),
        };
        Self {
            serial: SERIAL.fetch_add(1, Ordering::Relaxed),
            dropped,
            events,
            index: Arc::clone(index),
            state,
            selected: None,
        }
    }

    /// Returns a number no other soundbank preview of this process has.
    #[must_use]
    pub const fn serial(&self) -> u64 {
        self.serial
    }

    /// Returns where the preview stands.
    #[must_use]
    pub fn status(&self) -> SoundbankStatus<'_> {
        if self.index.watch().is_changed() {
            return SoundbankStatus::ArchiveChanged;
        }
        match &self.state {
            State::Opening => SoundbankStatus::Opening,
            State::TooLarge => SoundbankStatus::TooLarge,
            State::Failed(reason) => SoundbankStatus::Failed(reason.as_deref()),
            State::ArchiveChanged => SoundbankStatus::ArchiveChanged,
            State::Opened(opened) => SoundbankStatus::Opened(&opened.sounds),
        }
    }

    /// Returns original inputs for the displayed path, size and distinct embedded WEM count.
    #[must_use]
    pub fn summary_provenance(
        &self,
    ) -> Option<Result<&SoundbankSummaryProvenance, &SourceEvidenceError>> {
        Some(self.opened()?.summary.as_ref())
    }

    /// Returns the original inputs for a row of the current soundbank.
    #[must_use]
    pub fn row_provenance(
        &self,
        id: u32,
    ) -> Option<Result<&BankSoundProvenance, &WemHeaderEvidenceError>> {
        let opened = self.opened()?;
        let row = opened.evidence.get(opened.position(id)?)?;
        let wem = opened.bank.wem(id)?;
        Some(row.provenance(&wem, opened.original.as_deref()))
    }

    /// Returns the ID of the selected WEM.
    #[must_use]
    pub fn selected(&self) -> Option<u32> {
        self.opened()?;
        self.selected.as_ref().map(|selected| selected.id)
    }

    /// Returns the selected WEM.
    #[must_use]
    pub fn selected_sound(&self) -> Option<&BankSound> {
        let id = self.selected()?;
        self.sound(id)
    }

    /// Returns the audio preview of the selected WEM, or `None` when none is selected or the
    /// soundbank holds only its prefetched start.
    #[must_use]
    pub fn audio(&self) -> Option<&AudioPreview> {
        self.opened()?;
        self.selected.as_ref()?.audio.as_ref()
    }

    /// Returns the audio preview of the selected WEM for controlling its playback.
    pub fn audio_mut(&mut self) -> Option<&mut AudioPreview> {
        self.opened()?;
        self.selected.as_mut()?.audio.as_mut()
    }

    /// Selects the WEM `id`, unless it is selected already or the soundbank embeds none, and
    /// decodes it on a background thread; `wake` is called from there. Selecting another WEM
    /// stops the one playing.
    pub(crate) fn select(&mut self, id: u32, wake: impl Fn() + Send + 'static) {
        if self.selected() == Some(id) {
            return;
        }
        let Some(opened) = self.opened() else {
            return;
        };
        let Some(sound) = self.sound(id) else {
            return;
        };
        let audio = sound.is_playable().then(|| {
            AudioPreview::start_embedded(&opened.bank, id, opened.original.as_ref(), wake)
        });
        self.selected = Some(Selected { id, audio });
    }

    /// Takes the result of the background thread and of the audio preview of the selected WEM;
    /// returns whether one arrived.
    pub(crate) fn poll(&mut self) -> bool {
        if self.index.watch().is_changed() {
            let changed = !matches!(self.state, State::ArchiveChanged);
            self.dropped.store(true, Ordering::Release);
            self.apply(Event::ArchiveChanged);
            return changed;
        }
        let sounded = self.audio_mut().is_some_and(AudioPreview::poll);
        let mut changed = false;
        loop {
            match self.events.try_recv() {
                Ok(event) => {
                    self.apply(event);
                    changed = true;
                }
                Err(TryRecvError::Empty) => return changed || sounded,
                Err(TryRecvError::Disconnected) => {
                    if matches!(self.state, State::Opening) {
                        self.state = State::Failed(None);
                        changed = true;
                    }
                    return changed || sounded;
                }
            }
        }
    }

    fn sound(&self, id: u32) -> Option<&BankSound> {
        let opened = self.opened()?;
        opened.sounds.get(opened.position(id)?)
    }

    fn opened(&self) -> Option<&Opened> {
        match &self.state {
            State::Opened(opened) if !self.index.watch().is_changed() => Some(opened),
            _ => None,
        }
    }

    fn apply(&mut self, event: Event) {
        self.state = match event {
            Event::TooLarge => State::TooLarge,
            Event::Failed(reason) => State::Failed(reason),
            Event::ArchiveChanged => State::ArchiveChanged,
            Event::Opened(opened) => State::Opened(opened),
        };
        if !matches!(self.state, State::Opened(_)) {
            self.selected = None;
        }
    }
}

impl Drop for SoundbankPreview {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}
