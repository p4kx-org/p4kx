// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
#[cfg(target_os = "linux")]
use std::io::Read;
use std::io::Write;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "linux")]
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
    const COMPLETION_DEADLINE: Duration = Duration::from_secs(30);

    for expected in [Ok(42), Err(AliasUncertainty::AccessDenied)] {
        let (client, mut server) = sockets()?;
        let shutdown = client.try_clone()?;
        let called = Arc::new(AtomicBool::new(false));
        let watched = Arc::clone(&called);
        let result = super::with_socket_deadline(
            Instant::now() + COMPLETION_DEADLINE,
            &AtomicBool::new(false),
            move || {
                watched.store(true, Ordering::Release);
                shutdown.shutdown(Shutdown::Both)
            },
            || expected.map_err(UnknownAlias::new),
        );
        assert_eq!(
            result.map_err(|error| error.reason()),
            expected,
            "the work result is retained after the supervisor joins"
        );
        let called = Arc::try_unwrap(called)
            .map_err(|_| "the supervisor still owns the socket shutdown callback")?;
        assert!(
            !called.into_inner(),
            "completed work must leave the socket live"
        );
        server.write_all(b"alive")?;
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
