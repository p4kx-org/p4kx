// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::sync::mpsc;

use super::GameWatch;

#[test]
fn a_disconnected_probe_warns_and_schedules_a_retry() {
    let (sender, receiver) = mpsc::channel();
    drop(sender);
    let mut watch = GameWatch {
        enabled: true,
        pending: Some(receiver),
        ..GameWatch::default()
    };
    watch.poll(|| {});
    assert!(
        watch.running,
        "a disconnected worker cannot certify absence"
    );
    assert!(watch.pending.is_none(), "the failed worker is released");
    assert!(watch.next.is_some(), "ordinary polling can retry later");
}

#[test]
fn a_failed_probe_start_warns_without_leaving_a_pending_worker() {
    let mut watch = GameWatch {
        enabled: true,
        ..GameWatch::default()
    };
    watch.probe_started(Err(io::Error::other("synthetic worker start failure")));
    assert!(
        watch.running,
        "a worker start failure leaves process status unknown"
    );
    assert!(watch.pending.is_none(), "no worker was started");
    assert!(watch.next.is_some(), "the failed start schedules a retry");
}

#[test]
fn a_pending_probe_stays_nonblocking_and_keeps_its_previous_warning() {
    let (_sender, receiver) = mpsc::channel();
    let mut watch = GameWatch {
        enabled: true,
        running: true,
        pending: Some(receiver),
        ..GameWatch::default()
    };
    watch.poll(|| {});
    assert!(
        watch.running,
        "a pending probe does not erase the previous warning"
    );
    assert!(watch.pending.is_some(), "the live worker remains pending");
    assert!(watch.next.is_none(), "no replacement worker is scheduled");
}
