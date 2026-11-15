// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, OnceLock};
use std::thread;

use p4kx_extraction::{DataForge, Guid, LanguageTexts, TagResolver, TagTree, TextResolver};

use super::{RecordView, lines};
use crate::forge_cache::{ForgeSource, ReadFailure};
use crate::model::describe;
use crate::text_cache::TextFailure;
use crate::{EntryIndex, RecordId, TextLanguage};

/// A Record to read for the preview, as request `generation`, with its localisation keys
/// resolved in `language`.
#[derive(Debug, Clone, Copy)]
pub(super) struct Request {
    pub(super) generation: u64,
    pub(super) id: RecordId,
    pub(super) language: Option<TextLanguage>,
}

/// Why a Record could not be read for the preview.
#[derive(Debug)]
pub(super) enum Unread {
    Failed(String),
    ArchiveChanged,
}

pub(super) type Answer = (u64, Result<RecordView, Unread>);

/// A thread that keeps the DataForge of one Archive open and reads the Records the preview asks
/// for, only the newest of the requests waiting; it ends once this handle is dropped.
#[derive(Debug)]
pub(super) struct Worker {
    requests: Sender<Request>,
    answers: Receiver<Answer>,
    dropped: Arc<AtomicBool>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Relaxed);
    }
}

/// What the worker answered since the last poll.
pub(super) enum Answered {
    Nothing,
    Answer(Box<Answer>),
    Gone,
}

impl Worker {
    /// Starts the worker over the DataForge of `index`, read through the cache of the Archive,
    /// calling `wake` from it after each answer.
    pub(super) fn start(
        index: &Arc<EntryIndex>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        let (requests, waiting) = mpsc::channel();
        let (answer, answers) = mpsc::channel();
        let index = Arc::clone(index);
        let dropped = Arc::new(AtomicBool::new(false));
        let thread_dropped = Arc::clone(&dropped);
        thread::Builder::new()
            .name("record-preview".to_owned())
            .spawn(move || run(&index, &waiting, &answer, &*wake, &thread_dropped))
            .map_err(|error| describe(&error))?;
        Ok(Self {
            requests,
            answers,
            dropped,
        })
    }

    /// Asks for a Record; returns whether the worker still runs.
    pub(super) fn request(&self, request: Request) -> bool {
        self.requests.send(request).is_ok()
    }

    /// Takes the next answer.
    pub(super) fn poll(&self) -> Answered {
        match self.answers.try_recv() {
            Ok(answer) => Answered::Answer(Box::new(answer)),
            Err(TryRecvError::Empty) => Answered::Nothing,
            Err(TryRecvError::Disconnected) => Answered::Gone,
        }
    }
}

fn run(
    index: &EntryIndex,
    waiting: &Receiver<Request>,
    answer: &Sender<Answer>,
    wake: &dyn Fn(),
    dropped: &AtomicBool,
) {
    let mut first = None;
    loop {
        let reason = match index.forge().read(index, || {}) {
            Ok(source) => match source.entry.open() {
                Ok(forge) => {
                    let tags = SourceTags::new(&source, index, dropped);
                    return serve(waiting, answer, wake, first, |request| {
                        read(index, &forge, &tags, request)
                    });
                }
                Err(error) => Unread::Failed(describe(&error)),
            },
            Err(ReadFailure::ArchiveChanged(_)) => Unread::ArchiveChanged,
            Err(ReadFailure::Unreadable(reason)) => Unread::Failed(reason),
        };
        let Some(request) = first.take().or_else(|| next(waiting)) else {
            return;
        };
        if answer.send((request.generation, Err(reason))).is_err() {
            return;
        }
        wake();
        let Some(retry) = next(waiting) else {
            return;
        };
        first = Some(retry);
    }
}

/// The tag paths of the Archive's DataForge, from the tag tree its cache builds on the first
/// lookup. The build stops once the Archive changed or the worker was dropped, and a build that
/// stopped or failed is tried again by the next lookup.
struct SourceTags<'a> {
    source: &'a ForgeSource,
    index: &'a EntryIndex,
    dropped: &'a AtomicBool,
    tree: OnceLock<Arc<TagTree>>,
}

impl<'a> SourceTags<'a> {
    const fn new(source: &'a ForgeSource, index: &'a EntryIndex, dropped: &'a AtomicBool) -> Self {
        Self {
            source,
            index,
            dropped,
            tree: OnceLock::new(),
        }
    }

    fn tree(&self) -> Option<&TagTree> {
        if let Some(tree) = self.tree.get() {
            return Some(tree);
        }
        let stop = || self.dropped.load(Ordering::Relaxed) || self.index.watch().is_changed();
        let built = self.source.tags(&stop).ok().flatten()?;
        Some(self.tree.get_or_init(|| built))
    }
}

impl TagResolver for SourceTags<'_> {
    fn tag_path(&self, guid: Guid) -> Option<&str> {
        self.tree()?.tag_path(guid)
    }
}

fn read(
    index: &EntryIndex,
    forge: &DataForge<'_>,
    tags: &dyn TagResolver,
    request: Request,
) -> Result<RecordView, Unread> {
    let record = forge
        .record(request.id.0)
        .ok_or_else(|| Unread::Failed(format!("the DataForge has no Record {}", request.id.0)))?;
    let tree = record
        .tree()
        .map_err(|error| Unread::Failed(describe(&error)))?;
    let mut failure = None;
    let mut view = match request.language {
        None => lines::view(forge, &tree, None, tags),
        Some(language) => match index.texts().load(index, language) {
            Ok(texts) => {
                let resolver = LanguageTexts::owned(&texts, language);
                let texts = Some(&resolver as &dyn TextResolver);
                let mut view = lines::view(forge, &tree, texts, tags);
                view.language = Some(language);
                view
            }
            Err(error) => {
                let reason = match error {
                    TextFailure::ArchiveChanged => return Err(Unread::ArchiveChanged),
                    TextFailure::Unreadable(reason) => reason,
                    TextFailure::Reserve(error) => describe(&error),
                };
                failure = Some((language, reason.into_boxed_str()));
                lines::view(forge, &tree, None, tags)
            }
        },
    };
    view.texts_failure = failure;
    if index.watch().check().is_err() {
        return Err(Unread::ArchiveChanged);
    }
    Ok(view)
}

fn next(waiting: &Receiver<Request>) -> Option<Request> {
    let mut request = waiting.recv().ok()?;
    while let Ok(newer) = waiting.try_recv() {
        request = newer;
    }
    Some(request)
}

fn serve(
    waiting: &Receiver<Request>,
    answer: &Sender<Answer>,
    wake: &dyn Fn(),
    mut first: Option<Request>,
    mut read: impl FnMut(Request) -> Result<RecordView, Unread>,
) {
    while let Some(request) = first.take().or_else(|| next(waiting)) {
        if answer.send((request.generation, read(request))).is_err() {
            return;
        }
        wake();
    }
}
