// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use p4kx_channels::SearchPaths;

use super::{ChannelRefresh, Measurement, RefreshFailure, Request};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const WAIT: Duration = Duration::from_secs(10);

fn measured(name: &str) -> TestResult<Measurement> {
    let root = tempfile::tempdir()?;
    let folder = root.path().join(name);
    fs::create_dir_all(&folder)?;
    let archive = folder.join("Data.p4k");
    fs::write(&archive, b"")?;
    let measured = SearchPaths {
        manual_archives: vec![archive],
        ..SearchPaths::default()
    }
    .launcher_snapshot()
    .detect();
    assert!(
        measured.is_complete(),
        "the owned synthetic source has a complete measured result"
    );
    Ok(measured)
}

fn request(measured: Measurement) -> Request {
    Request {
        measure: Box::new(move || measured),
        wake: Box::new(|| {}),
    }
}

fn name(measured: &Measurement) -> TestResult<&str> {
    Ok(measured
        .value()
        .channels()
        .first()
        .ok_or("measured Channel")?
        .name())
}

#[test]
fn requesting_discovery_never_runs_the_measurement_on_the_calling_thread() -> TestResult {
    let measured = measured("WORKER")?;
    let (sender, receiver) = mpsc::channel();
    let wakes = Arc::new(AtomicUsize::new(0));
    let worker_wakes = Arc::clone(&wakes);
    let input = Request {
        measure: Box::new(move || {
            assert!(
                sender.send(thread::current().id()).is_ok(),
                "the owned caller receives the measurement thread identity"
            );
            measured
        }),
        wake: Box::new(move || {
            worker_wakes.fetch_add(1, Ordering::SeqCst);
        }),
    };
    let mut refresh = ChannelRefresh::default();
    let mut held = None;
    refresh.queue_using(input, |job| {
        held = Some(job);
        Ok(())
    });
    assert!(
        receiver.try_recv().is_err(),
        "requesting only hands work to the executor"
    );
    let worker = thread::spawn(held.take().ok_or("queued worker")?);
    assert_ne!(
        receiver.recv_timeout(WAIT)?,
        thread::current().id(),
        "the same launched measurement executes on a worker thread"
    );
    worker.join().map_err(|_| "synthetic worker stopped")?;
    let measured = refresh.poll().ok_or("completed worker report")?;
    assert_eq!(
        name(&measured)?,
        "WORKER",
        "poll returns the actual measured result"
    );
    assert_eq!(
        wakes.load(Ordering::SeqCst),
        1,
        "successful delivery wakes the UI exactly once"
    );
    Ok(())
}

#[test]
fn only_the_latest_pending_manual_state_runs_and_an_older_result_is_never_adopted() -> TestResult {
    let mut refresh = ChannelRefresh::default();
    let mut first = None;
    refresh.queue_using(request(measured("OLD")?), |job| {
        first = Some(job);
        Ok(())
    });
    let mut unexpected_launches = 0;
    refresh.queue_using(request(measured("MIDDLE")?), |_job| {
        unexpected_launches += 1;
        Ok(())
    });
    refresh.queue_using(request(measured("LATEST")?), |_job| {
        unexpected_launches += 1;
        Ok(())
    });
    assert_eq!(
        unexpected_launches, 0,
        "additional requests do not spawn parallel scans"
    );
    first.take().ok_or("first worker")?();
    let mut latest = None;
    let stale = refresh.poll_using(|job| {
        latest = Some(job);
        Ok(())
    });
    assert!(
        stale.is_none(),
        "the completed old result is discarded before it can replace the manual set"
    );
    latest.take().ok_or("latest worker")?();
    let measured = refresh.poll().ok_or("latest report")?;
    assert_eq!(
        name(&measured)?,
        "LATEST",
        "the overwritten middle request is never adopted"
    );
    assert!(
        refresh.running.is_none() && refresh.pending.is_none(),
        "the bounded queue is drained"
    );
    Ok(())
}

#[test]
fn replacing_a_channel_list_invalidates_its_running_result_without_a_stopped_notice() -> TestResult
{
    let mut refresh = ChannelRefresh::default();
    let mut held = None;
    refresh.queue_using(request(measured("OLD")?), |job| {
        held = Some(job);
        Ok(())
    });
    refresh.invalidate();
    held.take().ok_or("invalidated worker")?();
    assert!(
        refresh.poll().is_none(),
        "an invalidated result cannot populate the replacement list"
    );
    assert!(
        refresh.failure().is_none(),
        "discarding obsolete work is not a current discovery failure"
    );
    Ok(())
}

#[test]
fn spawn_failure_keeps_the_original_reason_and_a_healthy_retry_can_complete() -> TestResult {
    let mut refresh = ChannelRefresh::default();
    refresh.queue_using(request(measured("FAILED")?), |_job| {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "spawn sentinel",
        ))
    });
    assert!(
        matches!(refresh.failure(), Some(RefreshFailure::Spawn(error))
        if error.kind() == io::ErrorKind::PermissionDenied && error.to_string() == "spawn sentinel"),
        "the actual spawn branch retains the original I/O kind and reason"
    );
    let mut held = None;
    refresh.queue_using(request(measured("RECOVERED")?), |job| {
        held = Some(job);
        Ok(())
    });
    held.take().ok_or("retry worker")?();
    let measured = refresh.poll().ok_or("recovered report")?;
    assert_eq!(
        name(&measured)?,
        "RECOVERED",
        "a new manual request retries after a spawn failure"
    );
    assert!(
        refresh.failure().is_none(),
        "healthy recovery clears the prior worker notice"
    );
    Ok(())
}

#[test]
fn a_current_worker_disconnecting_without_a_report_remains_visible() -> TestResult {
    let mut refresh = ChannelRefresh::default();
    let wakes = Arc::new(AtomicUsize::new(0));
    let stopped_wakes = Arc::clone(&wakes);
    let mut input = request(measured("STOPPED")?);
    input.wake = Box::new(move || {
        stopped_wakes.fetch_add(1, Ordering::SeqCst);
    });
    refresh.queue_using(input, |_job| Ok(()));
    assert_eq!(
        wakes.load(Ordering::SeqCst),
        1,
        "dropping accepted work closes the sender and wakes the UI"
    );
    assert!(
        refresh.poll().is_none(),
        "disconnect does not forge a successful empty report"
    );
    assert!(
        matches!(refresh.failure(), Some(RefreshFailure::Stopped)),
        "the production receiver branch exposes the missing outcome distinctly"
    );
    Ok(())
}
