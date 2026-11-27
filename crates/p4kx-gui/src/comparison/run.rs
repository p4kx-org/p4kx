// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::thread;

use p4kx_comparison::{
    Archive, CancelToken, CompareOptions, ComparisonError, ComparisonProgress, DataForgeEntry,
    Levels, OwnedComparison, OwnedDataForges, RecompareError,
};

use p4kx_extraction::ArchiveKey;

use super::record_list::RecordChangeList;
use super::schema_view::SchemaView;
use super::{ComparisonStage, ComparisonView};
use crate::model::describe;
use crate::protected_open::ProtectedResult;

/// How a Comparison ended.
#[derive(Debug)]
pub(crate) enum Outcome {
    Finished(Box<ComparisonView>),
    Failed(Option<String>),
    Cancelled,
}

#[derive(Debug)]
pub(crate) struct Completion {
    pub(crate) outcome: Outcome,
    pub(crate) issues: Vec<p4kx_channels::DiscoveryIssue>,
}

/// A Comparison running on a background thread.
#[derive(Debug)]
pub(crate) struct Run {
    cancel: CancelToken,
    alive: Weak<()>,
    stage: Arc<Mutex<ComparisonStage>>,
    key_asked: Arc<AtomicBool>,
    result: Receiver<Completion>,
}

impl Run {
    /// Returns how far the Comparison has come.
    pub(crate) fn stage(&self) -> ComparisonStage {
        *self.stage.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Asks the thread to stop at its next check; returns a handle that lives as long as the
    /// thread.
    pub(crate) fn cancel(&self) -> Weak<()> {
        self.cancel.cancel();
        Weak::clone(&self.alive)
    }

    /// Returns whether an encrypted Entry was needed while an Archive had no key that fits.
    pub(crate) fn key_asked(&self) -> bool {
        self.key_asked.load(Ordering::Acquire)
    }

    /// Takes the outcome once the thread ended.
    pub(crate) fn poll(&self) -> Option<Completion> {
        match self.result.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Completion {
                outcome: Outcome::Failed(None),
                issues: Vec::new(),
            }),
        }
    }
}

/// Starts comparing the Archives at `old` and `new` on a background thread, calling `wake` from
/// it as it advances and once it ended.
pub(crate) fn start(
    (old, new): (PathBuf, PathBuf),
    levels: Levels,
    key: Option<ArchiveKey>,
    wake: impl Fn() + Send + Sync + 'static,
) -> Result<Run, String> {
    let cancel = CancelToken::new();
    let stage = Arc::new(Mutex::new(ComparisonStage::Opening));
    let (sender, result) = mpsc::channel();
    let thread_cancel = cancel.clone();
    let thread_stage = Arc::clone(&stage);
    let key_asked = Arc::new(AtomicBool::new(false));
    let thread_asked = Arc::clone(&key_asked);
    let alive = Arc::new(());
    let thread_alive = Arc::clone(&alive);
    thread::Builder::new()
        .name("comparison".to_owned())
        .spawn(move || {
            let outcome = run(
                (&old, &new),
                (levels, key.as_ref()),
                (&thread_cancel, &thread_stage, &thread_asked),
                &wake,
            );
            if sender.send(outcome).is_ok() {
                wake();
            }
            drop(thread_alive);
        })
        .map_err(|error| describe(&error))?;
    Ok(Run {
        cancel,
        alive: Arc::downgrade(&alive),
        stage,
        key_asked,
        result,
    })
}

fn open(path: &Path, key: Option<ArchiveKey>) -> ProtectedResult<Arc<Archive>, String> {
    ProtectedResult::read(path, || {
        Archive::open_with_key(path, key)
            .map(Arc::new)
            .map_err(|error| format!("{}: {}", path.display(), describe(&error)))
    })
}

fn open_both(
    old: &Path,
    new: &Path,
    key: Option<&ArchiveKey>,
) -> ProtectedResult<(Arc<Archive>, Arc<Archive>), String> {
    let (old, new) = rayon::join(|| open(old, key.cloned()), || open(new, key.cloned()));
    old.zip(new)
}

/// Reads the DataForge Entry of `archive` and checks that it opens.
fn read_forge(archive: &Archive) -> Result<Arc<DataForgeEntry>, String> {
    let fail = |reason: String| format!("{}: {reason}", archive.path().display());
    let entry = DataForgeEntry::read(archive).map_err(|error| fail(describe(&error)))?;
    entry.open().map_err(|error| fail(describe(&error)))?;
    Ok(Arc::new(entry))
}

fn read_forges(old: &Archive, new: &Archive) -> Result<OwnedDataForges, String> {
    let (old, new) = rayon::join(|| read_forge(old), || read_forge(new));
    Ok(OwnedDataForges {
        old: old?,
        new: new?,
    })
}

fn run(
    paths: (&Path, &Path),
    options: (Levels, Option<&ArchiveKey>),
    progress: (&CancelToken, &Mutex<ComparisonStage>, &AtomicBool),
    wake: &(impl Fn() + Sync),
) -> Completion {
    let opened = open_both(paths.0, paths.1, options.1);
    let outcome = compare(opened.result, options.0, progress, wake);
    Completion {
        outcome,
        issues: opened.issues,
    }
}

fn compare(
    archives: Result<(Arc<Archive>, Arc<Archive>), String>,
    levels: Levels,
    (cancel, stage, key_asked): (&CancelToken, &Mutex<ComparisonStage>, &AtomicBool),
    wake: &(impl Fn() + Sync),
) -> Outcome {
    let set = |now: ComparisonStage| {
        *stage.lock().unwrap_or_else(PoisonError::into_inner) = now;
        wake();
    };
    let (old, new) = match archives {
        Ok(both) => both,
        Err(reason) => return Outcome::Failed(Some(reason)),
    };
    let forges = match levels.records.then(|| read_forges(&old, &new)).transpose() {
        Ok(forges) => forges,
        Err(reason) => {
            key_asked.store(
                old.take_key_request() | new.take_key_request(),
                Ordering::Release,
            );
            return Outcome::Failed(Some(reason));
        }
    };
    if cancel.is_cancelled() {
        return Outcome::Cancelled;
    }
    let on_progress = |progress: ComparisonProgress| {
        set(ComparisonStage::Comparing {
            done: progress.entries_done,
            total: progress.entries_total,
        });
    };
    let patches = crate::public_patches::current();
    let options = CompareOptions {
        levels,
        filter: None,
        cancel: Some(cancel),
        progress: Some(&on_progress),
        public_patches: Some(&patches),
    };
    let comparison = match OwnedComparison::compare(old, new, forges, &options) {
        Ok(comparison) => comparison,
        Err(ComparisonError::Cancelled) => return Outcome::Cancelled,
        Err(error) => return Outcome::Failed(Some(describe(&error))),
    };
    prepare_view(comparison, levels.records, cancel, &set)
}

fn prepare_view(
    comparison: OwnedComparison,
    records_enabled: bool,
    cancel: &CancelToken,
    set: &(impl Fn(ComparisonStage) + Sync),
) -> Outcome {
    let records = match records_enabled.then(|| records(&comparison, cancel, set)) {
        None => None,
        Some(Ok(list)) => Some(list),
        Some(Err(outcome)) => return outcome,
    };
    if cancel.is_cancelled() {
        return Outcome::Cancelled;
    }
    set(ComparisonStage::Preparing);
    let schema = records.is_some().then(|| {
        comparison
            .schema_diff()
            .map(|diff| SchemaView::new(&diff))
            .map_err(|error| describe(&error))
    });
    ComparisonView::new(Arc::new(comparison), (records, schema), cancel)
        .map_or(Outcome::Cancelled, |view| Outcome::Finished(Box::new(view)))
}

/// Compares the Records of `comparison` field by field and lists the changes.
fn records(
    comparison: &OwnedComparison,
    cancel: &CancelToken,
    set: &(impl Fn(ComparisonStage) + Sync),
) -> Result<RecordChangeList, Outcome> {
    let on_progress = |progress: ComparisonProgress| {
        set(ComparisonStage::ComparingRecords {
            done: progress.records_done,
            total: progress.records_total,
        });
    };
    let listed = comparison.record_changes(None, Some(cancel), Some(&on_progress), |changes| {
        RecordChangeList::build(changes, cancel)
    });
    match listed {
        Ok(Some(list)) => Ok(list),
        Ok(None) | Err(RecompareError::Comparison(ComparisonError::Cancelled)) => {
            Err(Outcome::Cancelled)
        }
        Err(error) => Err(Outcome::Failed(Some(describe(&error)))),
    }
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
