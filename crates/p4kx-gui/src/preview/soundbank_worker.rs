// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

use p4kx_extraction::{EmbeddedWem, Soundbank, SourceUses};
use rayon::prelude::*;

use super::bank_sound::BankSound;
use super::entry_bytes::{self, Unreadable};
use super::soundbank::{Event, Opened};
use super::soundbank_provenance::SoundbankSource;
use crate::EntryIndex;
use crate::model::describe;

pub(super) struct Worker {
    pub(super) index: Arc<EntryIndex>,
    pub(super) events: Sender<Event>,
    pub(super) dropped: Arc<AtomicBool>,
}

impl Worker {
    pub(super) fn run(&self, id: u32, wake: &impl Fn()) {
        let event = self.open(id);
        if !self.dropped.load(Ordering::Acquire) && self.events.send(event).is_ok() {
            wake();
        }
    }

    fn open(&self, id: u32) -> Event {
        let (bytes, original) =
            match entry_bytes::read_whole_observed(&self.index, id, &self.dropped) {
                Ok(read) => read,
                Err(Unreadable::TooLarge) => return Event::TooLarge,
                Err(Unreadable::ArchiveChanged) => return Event::ArchiveChanged,
                Err(Unreadable::Failed(reason)) => return Event::Failed(reason),
            };
        let bank = match Soundbank::parse(bytes) {
            Ok(bank) => Arc::new(bank),
            Err(error) => return Event::Failed(Some(describe(&error))),
        };
        let uses = SourceUses::new();
        let summary =
            SoundbankSource::observe(&uses, original).and_then(|source| source.summary(&bank));
        let original = original.map(|entry| Arc::new(entry.snapshot()));
        let wems: Vec<EmbeddedWem<'_>> = bank.wems().collect();
        let (sounds, evidence) = wems.par_iter().map(BankSound::of).unzip();
        drop(wems);
        if self.index.watch().check().is_err() {
            return Event::ArchiveChanged;
        }
        Event::Opened(Box::new(Opened::new(
            bank, original, sounds, evidence, summary,
        )))
    }
}
