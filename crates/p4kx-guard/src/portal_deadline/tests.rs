// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
#[cfg(target_os = "linux")]
use std::io::Read;
use std::io::Write;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::portal_alias::{AliasUncertainty, UnknownAlias};

type TestResult = Result<(), Box<dyn Error>>;

fn sockets() -> std::io::Result<(TcpStream, TcpStream)> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let client = TcpStream::connect(listener.local_addr()?)?;
    let (server, _) = listener.accept()?;
    Ok((client, server))
}

#[test]
fn success_and_error_join_the_supervisor_without_a_late_shutdown() -> TestResult {
    for fails in [false, true] {
        let (client, mut server) = sockets()?;
        let shutdown = client.try_clone()?;
        let called = Arc::new(AtomicBool::new(false));
        let watched = Arc::clone(&called);
        let result = super::with_socket_deadline(
            Instant::now() + Duration::from_millis(40),
            &AtomicBool::new(false),
            move || {
                watched.store(true, Ordering::Release);
                shutdown.shutdown(Shutdown::Both)
            },
            || {
                if fails {
                    Err(UnknownAlias::new(AliasUncertainty::AccessDenied))
                } else {
                    Ok(42)
                }
            },
        );
        if fails {
            assert!(
                matches!(result, Err(error) if error.reason() == AliasUncertainty::AccessDenied),
                "the work error is retained after the supervisor joins"
            );
        } else {
            assert_eq!(result?, 42, "successful work is returned after joining");
        }
        thread::sleep(Duration::from_millis(60));
        server.write_all(b"alive")?;
        assert!(
            !called.load(Ordering::Acquire),
            "a detached watchdog would shut down the still-live socket after its deadline"
        );
    }
    Ok(())
}

#[test]
#[cfg(target_os = "linux")]
fn a_real_blocked_socket_read_is_interrupted_by_the_deadline() -> TestResult {
    let (mut client, _server) = std::os::unix::net::UnixStream::pair()?;
    let shutdown = client.try_clone()?;
    let start = Instant::now();
    let result = super::with_socket_deadline(
        start + Duration::from_millis(40),
        &AtomicBool::new(false),
        move || shutdown.shutdown(Shutdown::Both),
        || {
            let mut byte = [0_u8];
            client
                .read(&mut byte)
                .map_err(|error| UnknownAlias::with_source(AliasUncertainty::SessionBus, error))?;
            Ok(())
        },
    );
    assert!(
        matches!(result, Err(error) if error.reason() == AliasUncertainty::Timeout),
        "the socket deadline is distinct from a conclusive reply"
    );
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "the joined reader cannot remain blocked on a silent peer"
    );
    Ok(())
}

#[test]
#[cfg(target_os = "linux")]
fn cancellation_interrupts_a_real_blocked_socket_and_joins() -> TestResult {
    let (mut client, _server) = std::os::unix::net::UnixStream::pair()?;
    let shutdown = client.try_clone()?;
    let cancel = Arc::new(AtomicBool::new(false));
    let trigger = Arc::clone(&cancel);
    let cancellation = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        trigger.store(true, Ordering::Release);
    });
    let result = super::with_socket_deadline(
        Instant::now() + Duration::from_secs(2),
        &cancel,
        move || shutdown.shutdown(Shutdown::Both),
        || {
            let mut byte = [0_u8];
            client
                .read(&mut byte)
                .map_err(|error| UnknownAlias::with_source(AliasUncertainty::SessionBus, error))?;
            Ok(())
        },
    );
    cancellation
        .join()
        .map_err(|_| "cancellation trigger failed")?;
    assert!(
        matches!(result, Err(error) if error.reason() == AliasUncertainty::Cancelled),
        "cancellation is not converted to an allowed or timed-out alias"
    );
    Ok(())
}
