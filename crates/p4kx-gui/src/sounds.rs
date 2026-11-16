// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod export;
mod exporting;
mod language;
mod provenance;
mod resolve;
mod status;
mod view;
mod worker;
#[cfg(test)]
mod worker_tests;

use std::sync::Arc;

use p4kx_extraction::SourceEvidenceError;

pub use export::{SoundsExport, SoundsExportSetup, SoundsExportStatus, SoundsScope};
pub use provenance::{SoundBreakProvenance, SoundListProvenance, SoundVariantProvenance};
pub use status::{ListStatus, SoundsListStatus, SoundsStatus};
pub use view::{
    BreakLine, EntityInfo, EventLine, IndexInfo, ListSource, Playable, SoundList, TriggerList,
    TriggerRow, Variant,
};
pub(crate) use worker::EntityTarget;
pub use worker::MAX_SOUND_HITS;

use status::{Sounds, Stage};
use view::Playback;
use worker::{Answer, Answered, ListError, ListRequest, Request, Worker};

use crate::preview::BankCache;
use crate::{AudioPreview, EntryIndex};

/// The Sounds tab: the audio triggers of an Entity or of a search with their sounds, the playback
/// of one sound and the export of sounds.
#[derive(Debug, Default)]
pub struct SoundsTab {
    worker: Option<Worker>,
    stage: Stage,
    entity_text: String,
    query: String,
    list: Option<TriggerList>,
    list_error: Option<ListError>,
    listing: bool,
    pending_search: bool,
    list_generation: u64,
    list_serial: u64,
    selected: Option<usize>,
    sounds: Sounds,
    sounds_generation: u64,
    sounds_serial: u64,
    variant: Option<usize>,
    audio: Option<AudioPreview>,
    banks: BankCache,
    language: Option<String>,
    setup: Option<SoundsExportSetup>,
    export: Option<SoundsExport>,
    exports: u64,
}

impl SoundsTab {
    /// Returns the text of the Entity field.
    #[must_use]
    pub fn entity_text(&self) -> &str {
        &self.entity_text
    }

    /// Replaces the text of the Entity field.
    pub fn set_entity_text(&mut self, text: &str) {
        text.clone_into(&mut self.entity_text);
    }

    /// Returns the search text.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns where the audio index stands.
    #[must_use]
    pub fn status(&self) -> SoundsStatus<'_> {
        match &self.stage {
            Stage::Idle => SoundsStatus::Idle,
            Stage::Building => SoundsStatus::Building,
            Stage::Ready(info) => SoundsStatus::Ready(info),
            Stage::Failed(reason) => SoundsStatus::Failed(reason.as_deref()),
            Stage::ArchiveChanged => SoundsStatus::ArchiveChanged,
        }
    }

    /// Returns whether the audio was asked for.
    #[must_use]
    pub const fn is_started(&self) -> bool {
        !matches!(self.stage, Stage::Idle)
    }

    /// Returns where the trigger list stands.
    #[must_use]
    pub fn list_status(&self) -> ListStatus<'_> {
        if self.listing && self.list.is_none() {
            return ListStatus::Loading;
        }
        match (&self.list_error, &self.list) {
            (Some(ListError::NotAnEntity), _) => ListStatus::NotAnEntity,
            (Some(ListError::ArchiveChanged), _) => ListStatus::ArchiveChanged,
            (Some(ListError::Failed(reason)), _) => ListStatus::Failed(reason),
            (None, Some(list)) => ListStatus::Ready(list),
            (None, None) => ListStatus::Empty,
        }
    }

    /// Returns whether a list is being made.
    #[must_use]
    pub const fn is_listing(&self) -> bool {
        self.listing
    }

    /// Returns a number that changes with every list, for the caches of the drawn rows.
    #[must_use]
    pub const fn list_serial(&self) -> u64 {
        self.list_serial
    }

    /// Returns the position of the selected trigger in the list.
    #[must_use]
    pub const fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Returns where the sounds of the selected trigger stand.
    #[must_use]
    pub fn sounds_status(&self) -> SoundsListStatus<'_> {
        match &self.sounds {
            Sounds::None => SoundsListStatus::None,
            Sounds::Loading => SoundsListStatus::Loading,
            Sounds::Undefined => SoundsListStatus::Undefined,
            Sounds::Ready(list) => SoundsListStatus::Ready(&list.list),
        }
    }

    /// Returns the exact sound receipts or their preparation error for the current result.
    #[must_use]
    pub fn resolution_provenance(
        &self,
    ) -> Option<Result<&SoundListProvenance, SourceEvidenceError>> {
        let Sounds::Ready(result) = &self.sounds else {
            return None;
        };
        Some(result.provenance.as_ref().map_err(|error| *error))
    }

    /// Returns a number that changes with every resolved sound list.
    #[must_use]
    pub const fn sounds_serial(&self) -> u64 {
        self.sounds_serial
    }

    /// Returns the position of the selected variant in the sound list.
    #[must_use]
    pub const fn selected_variant(&self) -> Option<usize> {
        self.variant
    }

    /// Returns the playback of the selected variant, or `None` when none plays or is decoded.
    #[must_use]
    pub const fn audio(&self) -> Option<&AudioPreview> {
        self.audio.as_ref()
    }

    pub(crate) const fn audio_mut(&mut self) -> Option<&mut AudioPreview> {
        self.audio.as_mut()
    }

    /// Starts indexing the audio of `index` off the UI thread, calling `wake` from it as it
    /// advances; returns whether it started. Nothing starts when it is started already.
    pub(crate) fn load(
        &mut self,
        index: &Arc<EntryIndex>,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> bool {
        if self.is_started() {
            return false;
        }
        match Worker::start(index, Arc::new(wake)) {
            Ok(worker) => {
                self.worker = Some(worker);
                self.stage = Stage::Building;
            }
            Err(reason) => self.stage = Stage::Failed(Some(reason)),
        }
        true
    }

    /// Forgets the audio, the lists and the playback, because another Archive was opened.
    pub(crate) fn archive_opened(&mut self) {
        self.stop_export();
        *self = Self {
            entity_text: std::mem::take(&mut self.entity_text),
            exports: self.exports,
            ..Self::default()
        };
    }

    /// Drops the setup of a sound export and the sounds of the old Archive, because the Archive
    /// changed on disk.
    pub(crate) fn archive_changed(&mut self) {
        self.setup = None;
        self.audio = None;
        self.variant = None;
        self.banks.clear();
    }

    fn ask_list(&mut self, request: ListRequest) -> bool {
        let Some(worker) = &self.worker else {
            return false;
        };
        self.list_generation += 1;
        self.pending_search = matches!(request, ListRequest::Search(_));
        self.listing = worker.request(Request::List(self.list_generation, request));
        self.listing
    }

    /// Lists the audio triggers of the Entity `target` names; returns whether it was asked for.
    pub(crate) fn show_entity(&mut self, target: EntityTarget) -> bool {
        self.ask_list(ListRequest::Entity(target))
    }

    /// Lists the audio triggers of the Entity the Entity field names; returns whether it was
    /// asked for. An empty field asks for nothing.
    pub fn show_entity_text(&mut self) -> bool {
        let text = self.entity_text.trim().to_owned();
        !text.is_empty() && self.show_entity(EntityTarget::Text(text))
    }

    /// Searches the trigger names and the event names for `text`, a substring or a glob with
    /// `*` and `?`, ignoring case. An empty text drops the hits of a search.
    pub fn set_query(&mut self, text: &str) {
        text.clone_into(&mut self.query);
        if text.trim().is_empty() {
            if self.pending_search {
                self.list_generation += 1;
                self.pending_search = false;
                self.listing = false;
            }
            if self
                .list
                .as_ref()
                .is_some_and(|list| matches!(list.source, ListSource::Search(_)))
            {
                self.clear_list();
            }
            return;
        }
        self.ask_list(ListRequest::Search(text.trim().to_owned()));
    }

    fn clear_list(&mut self) {
        self.list = None;
        self.list_error = None;
        self.reset_selection();
    }

    fn reset_selection(&mut self) {
        self.sounds_generation += 1;
        self.selected = None;
        self.sounds = Sounds::None;
        self.variant = None;
        self.audio = None;
    }

    /// Selects the trigger at `position` of the list and resolves its sounds off the UI thread.
    pub fn select(&mut self, position: usize) {
        let Some(name) = self
            .list
            .as_ref()
            .and_then(|list| list.rows.get(position))
            .map(|row| row.name.clone())
        else {
            return;
        };
        if self.selected == Some(position) {
            return;
        }
        self.reset_selection();
        self.selected = Some(position);
        if let Some(worker) = &self.worker
            && worker.request(Request::Resolve(self.sounds_generation, name))
        {
            self.sounds = Sounds::Loading;
        }
    }

    /// Selects the variant at `position` of the sound list and decodes it for playing, unless it
    /// cannot be played.
    pub(crate) fn select_variant(
        &mut self,
        position: usize,
        index: &Arc<EntryIndex>,
        wake: impl Fn() + Send + 'static,
    ) {
        let Sounds::Ready(list) = &self.sounds else {
            return;
        };
        let Some(variant) = list.list.variants.get(position) else {
            return;
        };
        if self.variant == Some(position) {
            return;
        }
        self.audio = match &variant.playback {
            Some(Playback::Loose(id)) => Some(AudioPreview::start(index, *id, wake)),
            Some(Playback::Embedded { bank, wem }) => Some(AudioPreview::start_in_bank(
                index,
                (*bank, *wem),
                &self.banks,
                wake,
            )),
            None => None,
        };
        self.variant = Some(position);
    }

    /// Takes what the background thread and the playback produced; returns whether something
    /// changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = self.audio.as_mut().is_some_and(AudioPreview::poll);
        loop {
            let answered = match &self.worker {
                Some(worker) => worker.poll(),
                None => return changed,
            };
            match answered {
                Answered::Nothing => return changed,
                Answered::Answer(answer) => {
                    self.apply(*answer);
                    changed = true;
                }
                Answered::Gone => {
                    self.worker = None;
                    if matches!(self.stage, Stage::Building) {
                        self.stage = Stage::Failed(None);
                    }
                    self.listing = false;
                    if let Some(export) = &mut self.export {
                        export.lost();
                    }
                    return true;
                }
            }
        }
    }

    fn apply(&mut self, answer: Answer) {
        match answer {
            Answer::Ready(info) => self.stage = Stage::Ready(info),
            Answer::Unavailable => {
                self.stage = Stage::ArchiveChanged;
                self.worker = None;
            }
            Answer::List(generation, result) if generation == self.list_generation => {
                self.listing = false;
                self.pending_search = false;
                self.list_serial += 1;
                self.reset_selection();
                match result {
                    Ok(list) => {
                        self.list = Some(list);
                        self.list_error = None;
                    }
                    Err(error) => self.list_error = Some(error),
                }
            }
            Answer::Sounds(generation, result) if generation == self.sounds_generation => {
                self.sounds_serial += 1;
                self.sounds = match result {
                    Ok(list) => Sounds::Ready(list),
                    Err(_unknown) => Sounds::Undefined,
                };
            }
            Answer::Exported(id, result) => {
                if let Some(export) = self.export.as_mut().filter(|export| export.id() == id) {
                    export.finish(result);
                }
            }
            Answer::List(..) | Answer::Sounds(..) => {}
        }
    }
}
