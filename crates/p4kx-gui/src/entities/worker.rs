// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use p4kx_assembly::{ArchiveFiles, Entities, EntityNode, LoadoutChanges, LoadoutResolver};
use p4kx_extraction::Guid;

use super::resolve::{ResolveFailure, open_forge, read_source, resolve_in};
use super::rows::LoadoutView;
use crate::EntryIndex;

pub(super) type Resolved = Result<(LoadoutView, EntityNode), ResolveFailure>;

struct Job {
    number: u64,
    changes: LoadoutChanges,
    reply: Sender<Resolved>,
}

/// The thread that resolves the Loadout of one Entity for the lifetime of a session.
///
/// It opens the DataForge, indexes its Entities and keeps the Records it read once, and resolves
/// one set of changes after the other. A request supersedes the one before it, which stops at
/// its next port; dropping the worker stops the running one and ends the thread.
pub(super) struct Worker {
    jobs: Sender<Job>,
    latest: Arc<AtomicU64>,
    closed: Arc<AtomicBool>,
}

impl Worker {
    pub(super) fn spawn(
        index: &Arc<EntryIndex>,
        guid: Guid,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Self> {
        let (jobs, requests) = mpsc::channel();
        let latest = Arc::new(AtomicU64::new(0));
        let closed = Arc::new(AtomicBool::new(false));
        let thread_index = Arc::clone(index);
        let (thread_latest, thread_closed) = (Arc::clone(&latest), Arc::clone(&closed));
        thread::Builder::new()
            .name("loadout".to_owned())
            .spawn(move || {
                let flags = Flags {
                    latest: &thread_latest,
                    closed: &thread_closed,
                };
                serve(&thread_index, guid, &requests, flags, &*wake);
            })?;
        Ok(Self {
            jobs,
            latest,
            closed,
        })
    }

    /// Asks for the Loadout with `changes`, superseding the request before, and returns where the
    /// answer arrives.
    pub(super) fn request(&self, changes: LoadoutChanges) -> Receiver<Resolved> {
        let (reply, answer) = mpsc::channel();
        let number = self.latest.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
        let _ignored = self.jobs.send(Job {
            number,
            changes,
            reply,
        });
        answer
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        self.latest.fetch_add(1, Ordering::AcqRel);
    }
}

#[derive(Clone, Copy)]
struct Flags<'a> {
    latest: &'a AtomicU64,
    closed: &'a AtomicBool,
}

impl Flags<'_> {
    fn closed(self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

fn serve(
    index: &EntryIndex,
    guid: Guid,
    requests: &Receiver<Job>,
    flags: Flags<'_>,
    wake: &(dyn Fn() + Send + Sync),
) {
    let latest = flags.latest;
    let running = AtomicU64::new(0);
    let stop = || latest.load(Ordering::Acquire) != running.load(Ordering::Acquire);
    let source = match read_source(index) {
        Ok(source) => source,
        Err(failure) => return refuse(requests, &failure, wake),
    };
    if flags.closed() {
        return;
    }
    let forge = match open_forge(&source) {
        Ok(forge) => forge,
        Err(failure) => return refuse(requests, &failure, wake),
    };
    if flags.closed() {
        return;
    }
    let archive = match index.entries_archive() {
        Ok(archive) => archive,
        Err(error) => return refuse(requests, &error.into(), wake),
    };
    let files = ArchiveFiles::new(archive);
    let entities = Arc::new(Entities::new(&forge));
    if flags.closed() {
        return;
    }
    let mut resolver = LoadoutResolver::with_entities(entities)
        .with_forge_entry(&source.entry)
        .with_files(&files)
        .with_stop(&stop)
        .stopping_at_docked_vehicles();
    while let Some(job) = next(requests) {
        running.store(job.number, Ordering::Release);
        let outcome = resolve_in(index, &mut resolver, &forge, (guid, &job.changes))
            .map(|applied| (LoadoutView::of(&applied), applied.into_node()));
        if job.reply.send(outcome).is_ok() {
            wake();
        }
    }
}

fn refuse(requests: &Receiver<Job>, failure: &ResolveFailure, wake: &(dyn Fn() + Send + Sync)) {
    while let Some(job) = next(requests) {
        if job.reply.send(Err(failure.clone())).is_ok() {
            wake();
        }
    }
}

fn next(requests: &Receiver<Job>) -> Option<Job> {
    let mut job = requests.recv().ok()?;
    while let Ok(newer) = requests.try_recv() {
        job = newer;
    }
    Some(job)
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker").finish_non_exhaustive()
    }
}
