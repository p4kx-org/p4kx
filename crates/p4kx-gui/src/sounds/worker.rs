// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use p4kx_assembly::audio::{
    ArchiveAudio, AudioIndex, ExportOptions, ExportReport, ExportWatch, LanguageChoice,
    ResolveOptions, TriggerSounds,
};
use p4kx_assembly::{ENTITY_TYPE, Entities};
use p4kx_extraction::{CancelToken, ConversionError};

use super::provenance::{ResolvedSoundList, SoundListProvenance};
use super::resolve::{entity_rows, search_rows, sound_list};
use super::view::{EntityInfo, IndexInfo, ListSource, TriggerList};
use crate::EntryIndex;
use crate::forge_cache::ReadFailure;
use crate::model::describe;

/// The most hits of a search the list holds.
pub const MAX_SOUND_HITS: usize = 1_000;

/// The Entity a list is asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntityTarget {
    Record(u32),
    Text(String),
}

/// What an export is asked to write.
#[derive(Debug)]
pub(super) struct ExportRequest {
    pub(super) id: u64,
    pub(super) triggers: Vec<String>,
    pub(super) folder: PathBuf,
    pub(super) options: ExportOptions,
    pub(super) watch: Arc<ExportWatch>,
}

#[derive(Debug)]
pub(super) enum Request {
    List(u64, ListRequest),
    Resolve(u64, String),
    Language(Option<String>),
    Export(ExportRequest),
}

#[derive(Debug)]
pub(super) enum ListRequest {
    Entity(EntityTarget),
    Search(String),
}

/// Why a list could not be made.
#[derive(Debug)]
pub(super) enum ListError {
    NotAnEntity,
    ArchiveChanged,
    Failed(String),
}

/// What an export wrote and the triggers no ATL definition has.
pub(super) type Written = (ExportReport, Vec<String>);

#[derive(Debug)]
pub(super) enum Answer {
    Ready(IndexInfo),
    Unavailable,
    List(u64, Result<TriggerList, ListError>),
    Sounds(u64, Result<Box<ResolvedSoundList>, String>),
    Exported(u64, Result<Written, String>),
}

/// What the worker answered since the last poll.
pub(super) enum Answered {
    Nothing,
    Answer(Box<Answer>),
    Gone,
}

/// A thread that indexes the audio of one Archive, keeps the index and serves the lists, sounds
/// and exports asked of it; dropping this handle stops indexing and ends the thread.
#[derive(Debug)]
pub(super) struct Worker {
    requests: Sender<Request>,
    answers: Receiver<Answer>,
    cancel: CancelToken,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Worker {
    /// Starts indexing the audio of `index` and serving requests, calling `wake` from the thread
    /// after each answer.
    pub(super) fn start(
        index: &Arc<EntryIndex>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        let (requests, waiting) = mpsc::channel();
        let (answer, answers) = mpsc::channel();
        let index = Arc::clone(index);
        let cancel = CancelToken::new();
        let stop = cancel.clone();
        thread::Builder::new()
            .name("sounds".to_owned())
            .spawn(move || run(&index, (&waiting, &answer), &stop, &*wake))
            .map_err(|error| describe(&error))?;
        Ok(Self {
            requests,
            answers,
            cancel,
        })
    }

    #[cfg(test)]
    pub(super) fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }

    pub(super) fn request(&self, request: Request) -> bool {
        self.requests.send(request).is_ok()
    }

    pub(super) fn wait(&self) -> Answered {
        self.answers
            .recv()
            .map_or(Answered::Gone, |answer| Answered::Answer(Box::new(answer)))
    }

    pub(super) fn poll(&self) -> Answered {
        match self.answers.try_recv() {
            Ok(answer) => Answered::Answer(Box::new(answer)),
            Err(TryRecvError::Empty) => Answered::Nothing,
            Err(TryRecvError::Disconnected) => Answered::Gone,
        }
    }
}

type Wake<'a> = &'a (dyn Fn() + Send + Sync);

fn send(answers: &Sender<Answer>, wake: Wake<'_>, answer: Answer) -> bool {
    let sent = answers.send(answer).is_ok();
    if sent {
        wake();
    }
    sent
}

pub(super) fn run(
    index: &EntryIndex,
    (waiting, answers): (&Receiver<Request>, &Sender<Answer>),
    cancel: &CancelToken,
    wake: Wake<'_>,
) {
    let archive = index.watch().check().ok().and_then(|()| index.archive());
    let Some(archive) = archive else {
        send(answers, wake, Answer::Unavailable);
        return;
    };
    let Some(audio_index) = AudioIndex::build_until(archive, cancel) else {
        return;
    };
    let audio = ArchiveAudio::with_index(archive, &audio_index);
    let default_language = audio_index.default_language();
    let mut choice = LanguageChoice::new(None, default_language);
    let info = IndexInfo {
        triggers: audio_index.triggers().len(),
        soundbanks: audio_index.soundbanks().len(),
        languages: audio_index
            .languages()
            .iter()
            .map(ToString::to_string)
            .collect(),
        default_language: default_language.map(str::to_owned),
        issues: audio_index.issues().len(),
    };
    if !send(answers, wake, Answer::Ready(info)) {
        return;
    }
    thread::scope(|scope| {
        while let Some(batch) = next_batch(waiting) {
            for request in batch {
                let answer = match request {
                    Request::List(generation, list) => {
                        Answer::List(generation, make_list(index, &audio, list))
                    }
                    Request::Resolve(generation, trigger) => Answer::Sounds(
                        generation,
                        resolve(index, &audio, &choice, &trigger, cancel),
                    ),
                    Request::Language(name) => {
                        choice = LanguageChoice::new(name.as_deref(), default_language);
                        continue;
                    }
                    Request::Export(export) => {
                        let audio = &audio;
                        let choice = choice.clone();
                        let answers = answers.clone();
                        scope.spawn(move || {
                            let id = export.id;
                            let result = write(audio, &choice, &export);
                            send(&answers, wake, Answer::Exported(id, result));
                        });
                        continue;
                    }
                };
                if !send(answers, wake, answer) {
                    return;
                }
            }
        }
    });
}

/// Takes the waiting requests, keeping only the newest list and the newest Resolve request.
fn next_batch(waiting: &Receiver<Request>) -> Option<Vec<Request>> {
    let mut all = vec![waiting.recv().ok()?];
    while let Ok(next) = waiting.try_recv() {
        all.push(next);
    }
    let newest_list = all
        .iter()
        .rposition(|request| matches!(request, Request::List(..)));
    let newest_resolve = all
        .iter()
        .rposition(|request| matches!(request, Request::Resolve(..)));
    Some(
        all.into_iter()
            .enumerate()
            .filter(|(position, request)| match request {
                Request::List(..) => Some(*position) == newest_list,
                Request::Resolve(..) => Some(*position) == newest_resolve,
                Request::Language(_) | Request::Export(_) => true,
            })
            .map(|(_, request)| request)
            .collect(),
    )
}

fn make_list(
    index: &EntryIndex,
    audio: &ArchiveAudio<'_>,
    request: ListRequest,
) -> Result<TriggerList, ListError> {
    if index.watch().check().is_err() {
        return Err(ListError::ArchiveChanged);
    }
    match request {
        ListRequest::Search(text) => {
            let page = audio.index().search(&text, 0, MAX_SOUND_HITS);
            Ok(TriggerList {
                rows: search_rows(&page),
                total: page.total(),
                source: ListSource::Search(text),
            })
        }
        ListRequest::Entity(target) => entity_list(index, audio, &target),
    }
}

fn entity_list(
    index: &EntryIndex,
    audio: &ArchiveAudio<'_>,
    target: &EntityTarget,
) -> Result<TriggerList, ListError> {
    let failed = |error: &dyn std::error::Error| ListError::Failed(describe(error));
    let source = match index.forge().read(index, || {}) {
        Ok(source) => source,
        Err(ReadFailure::ArchiveChanged(_)) => return Err(ListError::ArchiveChanged),
        Err(ReadFailure::Unreadable(reason)) => return Err(ListError::Failed(reason)),
    };
    let forge = source.entry.open().map_err(|error| failed(&error))?;
    let record = match target {
        EntityTarget::Record(id) => forge
            .record(*id)
            .ok_or_else(|| ListError::Failed(format!("the DataForge has no Record {id}")))?,
        EntityTarget::Text(text) => Entities::new(&forge)
            .find(text)
            .map_err(|error| failed(&error))?,
    };
    if record.type_name().ok() != Some(ENTITY_TYPE) {
        return Err(ListError::NotAnEntity);
    }
    let resolved = audio
        .index()
        .entity_audio(&record)
        .map_err(|error| failed(&error))?;
    let name = record.name().unwrap_or_default();
    let info = EntityInfo {
        class_name: name
            .strip_prefix("EntityClassDefinition.")
            .unwrap_or(name)
            .to_owned(),
        guid: record.guid().to_string(),
        record_path: record.record_path().unwrap_or_default().to_owned(),
    };
    let rows = entity_rows(&resolved);
    Ok(TriggerList {
        total: rows.len(),
        rows,
        source: ListSource::Entity(info),
    })
}

fn resolve(
    index: &EntryIndex,
    audio: &ArchiveAudio<'_>,
    choice: &LanguageChoice,
    trigger: &str,
    cancel: &CancelToken,
) -> Result<Box<ResolvedSoundList>, String> {
    let options = ResolveOptions::default()
        .with_cancel(cancel)
        .with_path_sources();
    let sounds = audio
        .resolve_trigger_with_options(trigger, choice, options)
        .map_err(|error| describe(&error))?
        .ok_or_else(|| trigger.to_owned())?;
    Ok(Box::new(ResolvedSoundList {
        list: sound_list(index, &sounds),
        provenance: SoundListProvenance::capture(audio.archive(), audio.index(), &sounds),
    }))
}

fn write(
    audio: &ArchiveAudio<'_>,
    choice: &LanguageChoice,
    export: &ExportRequest,
) -> Result<Written, String> {
    let mut missing = Vec::new();
    let mut resolved: Vec<TriggerSounds<'_>> = Vec::new();
    let options = ResolveOptions::default().with_cancel(export.watch.cancel_token());
    for name in &export.triggers {
        match audio.resolve_trigger_with_options(name, choice, options) {
            Ok(Some(sounds)) => resolved.push(sounds),
            Ok(None) => missing.push(name.clone()),
            Err(ConversionError::Cancelled) => {
                return Ok((
                    ExportReport {
                        cancelled: true,
                        ..ExportReport::default()
                    },
                    missing,
                ));
            }
            Err(error) => return Err(describe(&error)),
        }
    }
    let triggers: Vec<&TriggerSounds<'_>> = resolved.iter().collect();
    let report = audio
        .export_sounds_watched(&triggers, &export.folder, export.options, &export.watch)
        .map_err(|error| describe(&error))?;
    Ok((report, missing))
}
