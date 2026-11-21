// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use p4kx_comparison::{OwnedComparison, RecordKey, RecordLookup};

use super::record_fields::RecordFields;
use crate::model::describe;

/// Where the field changes of the selected Record stand.
#[derive(Debug, Clone, Copy)]
pub enum RecordDetailStatus<'a> {
    /// They are compared off the UI thread.
    Loading,
    /// They are ready.
    Ready(&'a RecordFields),
    /// Comparing failed for the reason given, or stopped without a result.
    Failed(Option<&'a str>),
}

#[derive(Debug)]
enum State {
    Loading,
    Ready(RecordFields),
    Failed(Option<String>),
}

/// The field changes of one Record of the list.
#[derive(Debug)]
pub struct RecordDetail {
    row: u32,
    generation: u64,
    state: State,
}

impl RecordDetail {
    /// Returns the detail of row `row` that the request `generation` is comparing.
    pub(crate) const fn loading(row: u32, generation: u64) -> Self {
        Self {
            row,
            generation,
            state: State::Loading,
        }
    }

    /// Returns the detail of row `row` that failed for `reason`, or stopped without one.
    pub(crate) const fn failed(row: u32, generation: u64, reason: Option<String>) -> Self {
        Self {
            row,
            generation,
            state: State::Failed(reason),
        }
    }

    /// Returns the id of the row it compares.
    #[must_use]
    pub const fn row(&self) -> u32 {
        self.row
    }

    /// Returns the number of the request that compares it; no other detail of the view carries
    /// it.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns where it stands.
    #[must_use]
    pub fn status(&self) -> RecordDetailStatus<'_> {
        match &self.state {
            State::Loading => RecordDetailStatus::Loading,
            State::Ready(fields) => RecordDetailStatus::Ready(fields),
            State::Failed(reason) => RecordDetailStatus::Failed(reason.as_deref()),
        }
    }

    /// Takes the result of comparing it.
    pub(crate) fn finish(&mut self, result: Result<RecordFields, String>) {
        self.state = match result {
            Ok(fields) => State::Ready(fields),
            Err(reason) => State::Failed(Some(reason)),
        };
    }

    /// Marks it as stopped without a result unless it has one.
    pub(crate) fn stop(&mut self) {
        if matches!(self.state, State::Loading) {
            self.state = State::Failed(None);
        }
    }
}

type Answer = (u64, Result<RecordFields, String>);

/// A thread that keeps both DataForges open and compares the Records asked for, only the newest
/// of the requests waiting; it ends once this handle is dropped.
#[derive(Debug)]
pub(crate) struct RecordWorker {
    requests: Sender<(u64, RecordKey)>,
    answers: Receiver<Answer>,
}

/// What the worker answered since the last poll.
pub(crate) enum Answered {
    Nothing,
    Answer(Answer),
    Gone,
}

impl RecordWorker {
    /// Starts the worker over the DataForges of `comparison`, calling `wake` from it after each
    /// answer.
    pub(crate) fn start(
        comparison: &Arc<OwnedComparison>,
        wake: impl Fn() + Send + 'static,
    ) -> Result<Self, String> {
        let (requests, waiting) = mpsc::channel();
        let (answer, answers) = mpsc::channel();
        let thread_comparison = Arc::clone(comparison);
        thread::Builder::new()
            .name("comparison-records".to_owned())
            .spawn(move || {
                let served = thread_comparison.with_records(|lookup| {
                    serve(&waiting, &answer, &wake, |key| Ok(fields(lookup, key)));
                });
                if let Err(error) = served {
                    let reason = describe(&error);
                    serve(&waiting, &answer, &wake, |_| Err(reason.clone()));
                }
            })
            .map_err(|error| describe(&error))?;
        Ok(Self { requests, answers })
    }

    /// Asks for the field changes of the Records at `key` as request `generation`; returns
    /// whether the worker still runs.
    pub(crate) fn request(&self, generation: u64, key: RecordKey) -> bool {
        self.requests.send((generation, key)).is_ok()
    }

    /// Takes the next answer.
    pub(crate) fn poll(&self) -> Answered {
        match self.answers.try_recv() {
            Ok(answer) => Answered::Answer(answer),
            Err(TryRecvError::Empty) => Answered::Nothing,
            Err(TryRecvError::Disconnected) => Answered::Gone,
        }
    }
}

fn fields(lookup: &RecordLookup<'_>, key: RecordKey) -> RecordFields {
    lookup
        .change(key)
        .map_or_default(|change| RecordFields::of(&change))
}

fn serve(
    waiting: &Receiver<(u64, RecordKey)>,
    answer: &Sender<Answer>,
    wake: &impl Fn(),
    mut compare: impl FnMut(RecordKey) -> Result<RecordFields, String>,
) {
    while let Ok(mut request) = waiting.recv() {
        while let Ok(newer) = waiting.try_recv() {
            request = newer;
        }
        let (generation, key) = request;
        if answer.send((generation, compare(key))).is_err() {
            return;
        }
        wake();
    }
}
