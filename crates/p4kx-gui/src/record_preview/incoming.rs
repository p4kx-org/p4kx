// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeOrigin, FieldOrigin, ReferenceIndex, SourceEvidenceError,
    SourceInventory,
};

#[path = "incoming_origins.rs"]
mod origins;

use super::{FieldLine, LineValue, RecordLink};
use crate::forge_cache::ReadFailure;
use crate::model::describe;
use crate::{EntryIndex, RecordFieldProvenance, RecordId};

/// The most References the "Referenced by" list shows; the count line still counts all of them.
pub const MAX_INCOMING_LINES: usize = 10_000;

static GENERATIONS: AtomicU64 = AtomicU64::new(1);

/// The References to one Record as the "Referenced by" list shows them: one line per Reference
/// with the field that holds it and the Record that holds it as a link.
#[derive(Debug)]
pub struct IncomingView {
    lines: Vec<FieldLine>,
    references: usize,
    records: usize,
    incomplete: bool,
    evidence: Result<origins::Evidence, SourceEvidenceError>,
}

impl IncomingView {
    /// Returns the original Reference sites and displayed Record member origins in line order.
    #[must_use]
    pub fn line_origins(&self) -> Option<&[RecordFieldProvenance]> {
        self.evidence
            .as_ref()
            .ok()
            .map(|proof| proof.rows.as_slice())
    }

    /// Returns the original scan used by the incoming Reference and referring Record counts.
    #[must_use]
    pub fn count_origin(&self) -> Option<&FieldOrigin> {
        self.evidence.as_ref().ok().map(|proof| &proof.count)
    }

    /// Returns the physical inputs and exact uses of the incoming summary and lines.
    ///
    /// # Errors
    /// Returns the error preventing the retained original observations from being bound.
    pub fn source_inventory(&self) -> Result<&SourceInventory, &SourceEvidenceError> {
        self.evidence.as_ref().map(|proof| &proof.inventory)
    }

    /// Returns the lines in the order of the referring Records and of the fields in each; the
    /// field is the name of a line, the referring Record its [`LineValue::Reference`].
    #[must_use]
    pub fn lines(&self) -> &[FieldLine] {
        &self.lines
    }

    /// Returns the number of References to the Record, also those past
    /// [`MAX_INCOMING_LINES`] that have no line.
    #[must_use]
    pub const fn references(&self) -> usize {
        self.references
    }

    /// Returns the number of Records that hold at least one of the References.
    #[must_use]
    pub const fn records(&self) -> usize {
        self.records
    }

    /// Returns whether the list shows fewer lines than there are References.
    #[must_use]
    pub const fn is_truncated(&self) -> bool {
        self.lines.len() < self.references
    }

    /// Returns whether some Records could not be indexed or the index is full, so that the list
    /// may miss References.
    #[must_use]
    pub const fn is_incomplete(&self) -> bool {
        self.incomplete
    }
}

/// Where finding the Records that refer to the previewed Record stands.
#[derive(Debug, Clone, Copy)]
pub enum IncomingStatus<'a> {
    /// The Records are found off the UI thread.
    Loading,
    /// The Records were found.
    Ready(&'a IncomingView),
    /// They cannot be found, for the reason given, or finding stopped without a result.
    Failed(Option<&'a str>),
    /// The Archive changed on disk, so they were not found.
    ArchiveChanged,
}

#[derive(Debug)]
enum State {
    Loading,
    Ready(Box<IncomingView>),
    Failed(Option<String>),
    ArchiveChanged,
}

/// The "Referenced by" list of one Record.
#[derive(Debug)]
pub struct IncomingPreview {
    record: RecordId,
    generation: u64,
    state: State,
}

impl IncomingPreview {
    /// Returns the Record whose referrers are listed.
    #[must_use]
    pub const fn record(&self) -> RecordId {
        self.record
    }

    /// Returns a number no other list of this process has, for caches.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns where the list stands.
    #[must_use]
    pub fn status(&self) -> IncomingStatus<'_> {
        match &self.state {
            State::Loading => IncomingStatus::Loading,
            State::Ready(view) => IncomingStatus::Ready(view),
            State::Failed(reason) => IncomingStatus::Failed(reason.as_deref()),
            State::ArchiveChanged => IncomingStatus::ArchiveChanged,
        }
    }
}

enum Unread {
    Failed(String),
    ArchiveChanged,
}

type Answer = (u64, Result<IncomingView, Unread>);

/// A Record whose referrers are asked for, as request `generation`.
struct Request {
    generation: u64,
    id: RecordId,
}

/// The one thread that finds the lists, the newest request first; it ends once this handle is
/// dropped, and stops building the index of References that moment.
#[derive(Debug)]
struct Worker {
    requests: Sender<Request>,
    cancel: CancelToken,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Worker {
    fn start(
        index: &Arc<EntryIndex>,
        answers: Sender<Answer>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        let (requests, waiting) = mpsc::channel();
        let cancel = CancelToken::new();
        let index = Arc::clone(index);
        let stop = cancel.clone();
        thread::Builder::new()
            .name("record-incoming".to_owned())
            .spawn(move || serve(&index, &waiting, &answers, &*wake, &stop))
            .map_err(|error| describe(&error))?;
        Ok(Self { requests, cancel })
    }
}

fn serve(
    index: &EntryIndex,
    waiting: &Receiver<Request>,
    answers: &Sender<Answer>,
    wake: &dyn Fn(),
    cancel: &CancelToken,
) {
    while let Some(request) = next(waiting) {
        let Some(found) = find(index, request.id, cancel) else {
            return;
        };
        if answers.send((request.generation, found)).is_err() {
            return;
        }
        wake();
    }
}

fn next(waiting: &Receiver<Request>) -> Option<Request> {
    let mut request = waiting.recv().ok()?;
    while let Ok(newer) = waiting.try_recv() {
        request = newer;
    }
    Some(request)
}

/// The list for the selected Record and the thread that finds it; the first search of an Archive
/// builds its index of References, and a Record selected meanwhile waits for it.
#[derive(Debug)]
pub(crate) struct IncomingList {
    preview: Option<IncomingPreview>,
    worker: Option<Worker>,
    answers: Sender<Answer>,
    arrivals: Receiver<Answer>,
}

impl Default for IncomingList {
    fn default() -> Self {
        let (answers, arrivals) = mpsc::channel();
        Self {
            preview: None,
            worker: None,
            answers,
            arrivals,
        }
    }
}

impl IncomingList {
    /// Returns the list of the last Record asked for.
    pub(crate) const fn preview(&self) -> Option<&IncomingPreview> {
        self.preview.as_ref()
    }

    /// Returns whether the list of `id` is shown, found or being found; a list that could not be
    /// found is asked for again once another Record was asked for in between.
    pub(crate) fn requests(&self, id: RecordId) -> bool {
        self.preview
            .as_ref()
            .is_some_and(|shown| shown.record == id)
    }

    /// Asks the worker thread, started by the first request, for the Records that refer to `id`,
    /// calling `wake` from it once it has them.
    pub(crate) fn request(
        &mut self,
        id: RecordId,
        index: &Arc<EntryIndex>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) {
        let generation = GENERATIONS.fetch_add(1, Ordering::Relaxed);
        let mut preview = IncomingPreview {
            record: id,
            generation,
            state: State::Loading,
        };
        if self.worker.is_none() {
            match Worker::start(index, self.answers.clone(), wake) {
                Ok(worker) => self.worker = Some(worker),
                Err(reason) => preview.state = State::Failed(Some(reason)),
            }
        }
        if let Some(worker) = &self.worker
            && worker.requests.send(Request { generation, id }).is_err()
        {
            self.worker = None;
            preview.state = State::Failed(None);
        }
        self.preview = Some(preview);
    }

    /// Takes the lists the threads found; returns whether the list shown changed.
    pub(crate) fn poll(&mut self) -> bool {
        let mut changed = false;
        loop {
            match self.arrivals.try_recv() {
                Ok((generation, found)) => {
                    if let Some(preview) = &mut self.preview
                        && preview.generation == generation
                    {
                        preview.state = match found {
                            Ok(view) => State::Ready(Box::new(view)),
                            Err(Unread::Failed(reason)) => State::Failed(Some(reason)),
                            Err(Unread::ArchiveChanged) => State::ArchiveChanged,
                        };
                        changed = true;
                    }
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return changed,
            }
        }
    }

    /// Drops the list, because the Archive changed on disk.
    pub(crate) fn archive_changed(&mut self) {
        if let Some(preview) = &mut self.preview {
            preview.state = State::ArchiveChanged;
        }
    }
}

/// Finds the list of `id`, or returns `None` once `cancel` stopped the work.
fn find(
    index: &EntryIndex,
    id: RecordId,
    cancel: &CancelToken,
) -> Option<Result<IncomingView, Unread>> {
    let unread = |failure: ReadFailure| match failure {
        ReadFailure::ArchiveChanged(_) => Unread::ArchiveChanged,
        ReadFailure::Unreadable(reason) => Unread::Failed(reason),
    };
    let source = match index.forge().read(index, || {}) {
        Ok(source) => source,
        Err(failure) => return Some(Err(unread(failure))),
    };
    let references = match source.references(&|| cancel.is_cancelled()) {
        Ok(Some(references)) => references,
        Ok(None) => return None,
        Err(failure) => return Some(Err(unread(failure))),
    };
    let forge = match source.entry.open() {
        Ok(forge) => forge,
        Err(error) => return Some(Err(Unread::Failed(describe(&error)))),
    };
    let view = view(&forge, &references, id, source.entry.origin_of(&forge));
    if index.watch().check().is_err() {
        return Some(Err(Unread::ArchiveChanged));
    }
    Some(Ok(view))
}

fn view(
    forge: &DataForge<'_>,
    references: &ReferenceIndex,
    id: RecordId,
    input: Option<&DataForgeOrigin>,
) -> IncomingView {
    let total = references.incoming(id.0).len();
    let mut evidence = origins::Capture::new(input);
    let mut lines = Vec::with_capacity(total.min(MAX_INCOMING_LINES));
    let mut records = 0;
    let mut last = None;
    for reference in references.incoming(id.0) {
        let source = reference.source();
        if last != Some(source) {
            records += 1;
            last = Some(source);
        }
        if lines.len() >= MAX_INCOMING_LINES {
            continue;
        }
        let Some(record) = forge.record(source) else {
            continue;
        };
        let path = record.record_path().ok();
        let kind = record.type_name().ok();
        if let Ok(capture) = &mut evidence
            && let Err(error) =
                capture.row(forge, references.origin(reference), &record, path, kind)
        {
            evidence = Err(error);
        }
        lines.push(FieldLine {
            depth: 0,
            name: reference.location().into(),
            value: LineValue::Reference(RecordLink {
                guid: record.guid(),
                record_path: path.unwrap_or_default().into(),
                type_name: kind.unwrap_or_default().into(),
                at: None,
                tag: None,
                target: Some(RecordId(source)),
            }),
        });
    }
    lines.shrink_to_fit();
    IncomingView {
        lines,
        references: total,
        records,
        incomplete: references.is_truncated() || references.unreadable_records() > 0,
        evidence: evidence.and_then(|capture| capture.finish(forge, references, id.0)),
    }
}
