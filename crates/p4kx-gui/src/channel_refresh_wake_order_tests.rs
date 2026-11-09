// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use p4kx_channels::{DiscoveryReport, SearchPaths, SearchRecipe};
use p4kx_settings::ManualChannels;

use super::{ChannelRefresh, ChannelSearch, RefreshFailure, Request, Work};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
type RefreshController = Arc<Mutex<ChannelRefresh>>;
type HeldWork = Arc<Mutex<Option<Work>>>;

#[derive(Debug)]
struct WakeState {
    channel: Option<String>,
    complete: bool,
    stopped: bool,
    queue: QueueState,
}

#[derive(Debug, PartialEq, Eq)]
enum QueueState {
    Idle,
    Pending,
    Running,
    RunningWithPending,
}

fn hold(work: Work, slot: &HeldWork) -> io::Result<()> {
    let mut slot = slot
        .lock()
        .map_err(|error| io::Error::other(error.to_string()))?;
    if slot.is_some() {
        return Err(io::Error::other(
            "a second worker was launched before the first was taken",
        ));
    }
    *slot = Some(work);
    drop(slot);
    Ok(())
}

fn take(slot: &HeldWork) -> TestResult<Work> {
    slot.lock()
        .map_err(|error| io::Error::other(error.to_string()))?
        .take()
        .ok_or_else(|| io::Error::other("queued discovery work is missing").into())
}

fn empty_search() -> ChannelSearch {
    ChannelSearch {
        recipe: Arc::new(SearchRecipe::explicit(SearchPaths::default())),
        manual: ManualChannels::default(),
    }
}

fn search(archive: PathBuf) -> ChannelSearch {
    ChannelSearch {
        manual: ManualChannels {
            folders: Vec::new(),
            archives: vec![archive],
        },
        ..empty_search()
    }
}

fn inspect_at_wake(controller: &RefreshController, next: &HeldWork) -> Result<WakeState, String> {
    let mut refresh = controller.lock().map_err(|error| error.to_string())?;
    let measured = refresh.poll_using(|work| hold(work, next));
    Ok(WakeState {
        channel: measured
            .as_ref()
            .and_then(|report| report.value().channels().first())
            .map(|channel| channel.name().to_owned()),
        complete: measured.as_ref().is_some_and(DiscoveryReport::is_complete),
        stopped: matches!(refresh.failure(), Some(RefreshFailure::Stopped)),
        queue: match (refresh.running.is_some(), refresh.pending.is_some()) {
            (false, false) => QueueState::Idle,
            (false, true) => QueueState::Pending,
            (true, false) => QueueState::Running,
            (true, true) => QueueState::RunningWithPending,
        },
    })
}

fn observer(
    controller: &RefreshController,
    next: &HeldWork,
    events: Sender<Result<WakeState, String>>,
) -> impl Fn() + Send + Sync + 'static {
    let controller = Arc::clone(controller);
    let next = Arc::clone(next);
    move || {
        let observed = inspect_at_wake(&controller, &next);
        let _receiver_gone = events.send(observed);
    }
}

fn queue(
    controller: &RefreshController,
    search: ChannelSearch,
    wake: impl Fn() + Send + Sync + 'static,
    slot: &HeldWork,
) -> TestResult {
    controller
        .lock()
        .map_err(|error| io::Error::other(error.to_string()))?
        .request_with(search, wake, |work| hold(work, slot));
    Ok(())
}

#[derive(Clone, Copy)]
enum Exit {
    Dropped,
    Unwound,
}

fn assert_stopped_at_first_wake(exit: Exit) -> TestResult {
    let controller = Arc::new(Mutex::new(ChannelRefresh::default()));
    let held = Arc::new(Mutex::new(None));
    let next = Arc::new(Mutex::new(None));
    let (events, received) = mpsc::channel();
    let wake = observer(&controller, &next, events);
    match exit {
        Exit::Dropped => queue(&controller, empty_search(), wake, &held)?,
        Exit::Unwound => {
            controller
                .lock()
                .map_err(|error| io::Error::other(error.to_string()))?
                .queue_using(
                    Request {
                        measure: Box::new(|| panic!("synthetic discovery unwind")),
                        wake: Box::new(wake),
                    },
                    |work| hold(work, &held),
                );
        }
    }
    let work = take(&held)?;
    match exit {
        Exit::Dropped => drop(work),
        Exit::Unwound => {
            let worker = thread::Builder::new()
                .name("test-discovery-unwind".to_owned())
                .spawn(work)?;
            assert!(
                worker.join().is_err(),
                "the producer really unwinds outside the controller lock"
            );
        }
    }
    let observed = received.try_recv()?.map_err(io::Error::other)?;
    assert!(
        observed.stopped,
        "the first completion wake must already observe the disconnected sender: {observed:?}"
    );
    assert!(
        observed.queue == QueueState::Idle && observed.channel.is_none(),
        "the same wake clears stopped work without inventing a measurement: {observed:?}"
    );
    assert!(
        received.try_recv().is_err(),
        "one producer exit issues exactly one completion wake"
    );
    Ok(())
}

#[test]
fn a_dropped_worker_is_visible_at_its_first_completion_wake() -> TestResult {
    assert_stopped_at_first_wake(Exit::Dropped)
}

#[test]
fn an_unwound_worker_is_visible_at_its_first_completion_wake() -> TestResult {
    assert_stopped_at_first_wake(Exit::Unwound)
}

#[test]
fn the_first_stopped_wake_starts_the_latest_pending_request() -> TestResult {
    let root = tempfile::tempdir()?;
    let folder = root.path().join("LATEST");
    fs::create_dir_all(&folder)?;
    let archive = folder.join("Data.p4k");
    fs::write(&archive, b"")?;
    let controller = Arc::new(Mutex::new(ChannelRefresh::default()));
    let held = Arc::new(Mutex::new(None));
    let next = Arc::new(Mutex::new(None));
    let (events, received) = mpsc::channel();
    queue(
        &controller,
        empty_search(),
        observer(&controller, &next, events.clone()),
        &held,
    )?;
    queue(
        &controller,
        search(archive),
        observer(&controller, &next, events),
        &held,
    )?;
    {
        let refresh = controller
            .lock()
            .map_err(|error| io::Error::other(error.to_string()))?;
        assert!(
            refresh.running.is_some() && refresh.pending.is_some(),
            "the real controller holds running and pending work"
        );
        drop(refresh);
    }
    drop(take(&held)?);
    let first = received.try_recv()?.map_err(io::Error::other)?;
    assert!(
        first.queue == QueueState::Running && !first.stopped && first.channel.is_none(),
        "the first stopped wake must launch the pending request immediately: {first:?}"
    );
    let work = take(&next)?;
    let worker = thread::Builder::new()
        .name("test-discovery-pending".to_owned())
        .spawn(work)?;
    worker
        .join()
        .map_err(|_| io::Error::other("the pending producer unexpectedly unwound"))?;
    let latest = received.try_recv()?.map_err(io::Error::other)?;
    assert_eq!(
        latest.channel.as_deref(),
        Some("LATEST"),
        "the actual pending producer delivers its measured Channel"
    );
    assert!(
        latest.complete && !latest.stopped && latest.queue == QueueState::Idle,
        "the completion wake adopts only the healthy latest result: {latest:?}"
    );
    assert!(
        received.try_recv().is_err(),
        "the two producer exits issue exactly two wakes"
    );
    Ok(())
}

#[test]
fn a_healthy_measurement_is_visible_at_its_completion_wake() -> TestResult {
    let root = tempfile::tempdir()?;
    let folder = root.path().join("HEALTHY");
    fs::create_dir_all(&folder)?;
    let archive = folder.join("Data.p4k");
    fs::write(&archive, b"")?;
    let controller = Arc::new(Mutex::new(ChannelRefresh::default()));
    let held = Arc::new(Mutex::new(None));
    let next = Arc::new(Mutex::new(None));
    let (events, received) = mpsc::channel();
    queue(
        &controller,
        search(archive),
        observer(&controller, &next, events),
        &held,
    )?;
    let work = take(&held)?;
    let worker = thread::Builder::new()
        .name("test-discovery-healthy".to_owned())
        .spawn(work)?;
    worker
        .join()
        .map_err(|_| io::Error::other("the healthy producer unexpectedly unwound"))?;
    let observed = received.try_recv()?.map_err(io::Error::other)?;
    assert_eq!(
        observed.channel.as_deref(),
        Some("HEALTHY"),
        "the real producer's report is already visible at its wake"
    );
    assert!(
        observed.complete && !observed.stopped && observed.queue == QueueState::Idle,
        "the healthy control drains the controller on its completion wake: {observed:?}"
    );
    assert!(
        received.try_recv().is_err(),
        "healthy delivery issues exactly one wake"
    );
    Ok(())
}
