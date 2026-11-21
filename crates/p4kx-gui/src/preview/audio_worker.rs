// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

use p4kx_extraction::{EmbeddedWem, EntryOrigin, Soundbank, Wem};

use super::audio::{Event, SoundHeader};
use super::bank_cache::BankCache;
use super::entry_bytes::{self, Unreadable};
use crate::EntryIndex;
use crate::audio::{self, Pcm, Undecoded};
use crate::model::describe;

/// Where the WEM of one audio preview comes from.
pub(super) enum Source {
    Entry(Arc<EntryIndex>, u32),
    Embedded {
        bank: Arc<Soundbank<Vec<u8>>>,
        id: u32,
        original: Option<Arc<EntryOrigin>>,
    },
    InBank(Arc<EntryIndex>, (u32, u32), BankCache),
}

/// What the background thread of one audio preview works with.
pub(super) struct Worker {
    source: Source,
    events: Sender<Event>,
    dropped: Arc<AtomicBool>,
}

impl Worker {
    pub(super) const fn new(
        source: Source,
        events: Sender<Event>,
        dropped: Arc<AtomicBool>,
    ) -> Self {
        Self {
            source,
            events,
            dropped,
        }
    }

    fn send(&self, event: Event, wake: &impl Fn()) -> bool {
        let sent = !self.dropped.load(Ordering::Acquire) && self.events.send(event).is_ok();
        if sent {
            wake();
        }
        sent
    }

    pub(super) fn run(&self, wake: &impl Fn()) {
        let event = match &self.source {
            Source::Entry(index, id) => self.run_entry(index, *id, wake),
            Source::Embedded { bank, id, original } => {
                self.run_embedded(bank, *id, wake, |wem, selected| {
                    SoundHeader::observed_embedded(wem, selected, original.as_deref())
                })
            }
            Source::InBank(index, ids, cache) => self.run_in_bank(index, (*ids, cache), wake),
        };
        if let Some(event) = event {
            self.send(event, wake);
        }
    }

    fn run_entry(&self, index: &EntryIndex, id: u32, wake: &impl Fn()) -> Option<Event> {
        let (bytes, source) = match entry_bytes::read_whole_observed(index, id, &self.dropped) {
            Ok(read) => read,
            Err(unread) => return Some(Event::from(unread)),
        };
        let wem = match Wem::parse(bytes) {
            Ok(wem) => wem,
            Err(error) => return Some(Event::Failed(Some(describe(&error)))),
        };
        let header = SoundHeader::observed(&wem, source);
        let decoded = self.decode(&wem, header, wake)?;
        drop(wem);
        Some(if index.watch().check().is_err() {
            Event::ArchiveChanged
        } else {
            Event::Decoded(decoded)
        })
    }

    fn run_in_bank(
        &self,
        index: &EntryIndex,
        ((bank, id), cache): ((u32, u32), &BankCache),
        wake: &impl Fn(),
    ) -> Option<Event> {
        let bank = match cache.get(bank) {
            Some(kept) => kept,
            None => match entry_bytes::read_whole(index, bank, &self.dropped) {
                Ok(bytes) => match Soundbank::parse(bytes) {
                    Ok(parsed) => {
                        let parsed = Arc::new(parsed);
                        cache.keep(bank, &parsed);
                        parsed
                    }
                    Err(error) => return Some(Event::Failed(Some(describe(&error)))),
                },
                Err(unread) => return Some(Event::from(unread)),
            },
        };
        let event = self.run_embedded(&bank, id, wake, |wem, _| SoundHeader::of(wem))?;
        Some(match event {
            Event::Decoded(_) if index.watch().check().is_err() => Event::ArchiveChanged,
            other => other,
        })
    }

    fn run_embedded(
        &self,
        bank: &Soundbank<Vec<u8>>,
        id: u32,
        wake: &impl Fn(),
        header: impl FnOnce(&Wem<&[u8]>, &EmbeddedWem<'_>) -> SoundHeader,
    ) -> Option<Event> {
        let Some(embedded) = bank.wem(id) else {
            return Some(Event::Failed(None));
        };
        match embedded.parse() {
            Ok(wem) => self
                .decode(&wem, header(&wem, &embedded), wake)
                .map(Event::Decoded),
            Err(error) => Some(Event::Failed(Some(describe(&error)))),
        }
    }

    fn decode<B: AsRef<[u8]>>(
        &self,
        wem: &Wem<B>,
        header: SoundHeader,
        wake: &impl Fn(),
    ) -> Option<Result<Pcm, Undecoded>> {
        self.send(Event::Opened(header), wake)
            .then(|| audio::decode(wem, &self.dropped))
    }
}

impl From<Unreadable> for Event {
    fn from(unread: Unreadable) -> Self {
        match unread {
            Unreadable::TooLarge => Self::TooLarge,
            Unreadable::ArchiveChanged => Self::ArchiveChanged,
            Unreadable::Failed(reason) => Self::Failed(reason),
        }
    }
}
