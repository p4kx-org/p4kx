// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;
use std::io::Read;
use std::net::Shutdown;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use zbus::blocking::MessageIterator;
use zbus::blocking::connection::Builder;
use zbus::connection::AuthMechanism;
use zbus::zvariant::OwnedValue;
use zbus::{Guid, Message};

use crate::portal_alias::{AliasUncertainty, UnknownAlias};
use crate::{portal_deadline, portal_reply};

use self::fixture_failure::FixtureFailure;

mod fixture_failure;

type TestError = Box<dyn Error + Send + Sync>;
type TestResult = Result<(), TestError>;

const CLIENT_DEADLINE: Duration = Duration::from_secs(2);
const SERVICE_DEADLINE: Duration = Duration::from_secs(3);
const METHOD_TIMEOUT: Duration = Duration::from_millis(200);

#[derive(Clone, Copy)]
enum Scenario {
    Probe,
    VersionFive,
    VersionFour,
    Denied,
    SilentCall,
    ClosedCall,
    ChangedOwner,
}

#[cfg(unix)]
type TestSocket = std::os::unix::net::UnixStream;
#[cfg(windows)]
type TestSocket = uds_windows::UnixStream;

#[cfg(unix)]
fn sockets() -> std::io::Result<(TestSocket, TestSocket)> {
    TestSocket::pair()
}

#[cfg(windows)]
fn sockets() -> std::io::Result<(TestSocket, TestSocket)> {
    TestSocket::pair()
}

fn builder(stream: TestSocket) -> Builder<'static> {
    Builder::async_io_unix_stream(stream)
}

fn serve(
    stream: TestSocket,
    scenario: Scenario,
    ready: &Sender<()>,
    finished: &Receiver<()>,
) -> Result<usize, TestError> {
    let stop = stream.try_clone()?;
    let deadline = Instant::now() + SERVICE_DEADLINE;
    let result = portal_deadline::with_socket_deadline(
        deadline,
        &AtomicBool::new(false),
        move || stop.shutdown(Shutdown::Both),
        || {
            let connection = builder(stream)
                .p2p()
                .server(Guid::generate())
                .map_err(portal_reply::call_error)?
                .auth_mechanism(AuthMechanism::Anonymous)
                .max_queued(1)
                .build()
                .map_err(portal_reply::call_error)?;
            let mut incoming = MessageIterator::from(&connection);
            // The receiver must be active before the client can send its first method.
            ready
                .send(())
                .map_err(|error| UnknownAlias::with_source(AliasUncertainty::SessionBus, error))?;
            let mut calls = 0;
            for index in 0..5 {
                let request = incoming
                    .next()
                    .ok_or_else(|| UnknownAlias::new(AliasUncertainty::SessionBus))?
                    .map_err(portal_reply::call_error)?;
                calls = index + 1;
                if matches!(scenario, Scenario::SilentCall) {
                    break;
                }
                if matches!(scenario, Scenario::ClosedCall) {
                    drop(incoming);
                    connection.close().map_err(portal_reply::call_error)?;
                    wait_finished(finished, deadline)?;
                    return Ok(calls);
                }
                let response =
                    response(&request, scenario, index).map_err(portal_reply::call_error)?;
                connection
                    .send(&response)
                    .map_err(portal_reply::call_error)?;
                if matches!(scenario, Scenario::VersionFour) && index == 1
                    || matches!(scenario, Scenario::Denied) && index == 3
                {
                    break;
                }
            }
            wait_finished(finished, deadline)?;
            drop(incoming);
            connection.close().map_err(portal_reply::call_error)?;
            Ok(calls)
        },
    );
    Ok(result?)
}

fn wait_finished(finished: &Receiver<()>, deadline: Instant) -> Result<(), UnknownAlias> {
    finished
        .recv_timeout(
            deadline
                .saturating_duration_since(Instant::now())
                .min(CLIENT_DEADLINE),
        )
        .map_err(|error| UnknownAlias::with_source(AliasUncertainty::SessionBus, error))
}

fn response(request: &Message, scenario: Scenario, index: usize) -> zbus::Result<Message> {
    let header = request.header();
    let member = header.member().map(zbus::names::MemberName::as_str);
    let sender = if member == Some("GetNameOwner") {
        super::BUS
    } else {
        ":1.7"
    };
    let reply = Message::method_return(&header)?.sender(sender)?;
    match member {
        Some("GetNameOwner") => reply.build(&if matches!(scenario, Scenario::ChangedOwner)
            && index == 4
        {
            ":1.8"
        } else {
            ":1.7"
        }),
        Some("Get") => {
            assert_eq!(
                header.destination().map(zbus::names::BusName::as_str),
                Some(super::DOCUMENTS),
                "the first property call permits standard bus autostart"
            );
            reply.build(&OwnedValue::from(
                if matches!(scenario, Scenario::VersionFour) {
                    4_u32
                } else {
                    5
                },
            ))
        }
        Some("GetMountPoint") => reply.build(&b"/run/user/1001/doc\0".to_vec()),
        Some("GetHostPaths") => {
            let requested: Vec<String> = request.body().deserialize()?;
            let expected = if matches!(scenario, Scenario::Probe) {
                Vec::new()
            } else {
                vec!["aabbccdd"]
            };
            assert_eq!(
                requested, expected,
                "the actual encoded query has its bounded document selection"
            );
            if matches!(scenario, Scenario::Denied) {
                Message::error(&header, "org.freedesktop.DBus.Error.AccessDenied")?
                    .sender(sender)?
                    .build(&"private /home/not-a-real-user/document")
            } else if matches!(scenario, Scenario::Probe) {
                reply.build(&BTreeMap::<String, Vec<u8>>::new())
            } else {
                reply.build(&BTreeMap::from([(
                    "aabbccdd",
                    b"/outside/exports\0".to_vec(),
                )]))
            }
        }
        _ => Err(zbus::Error::InvalidReply),
    }
}

fn query(
    scenario: Scenario,
) -> Result<(Result<super::DocumentsReply, UnknownAlias>, usize), TestError> {
    let (client, server) = sockets()?;
    let shutdown = client.try_clone()?;
    let (finished, observed) = mpsc::channel();
    let (ready, started) = mpsc::channel();
    thread::scope(|scope| {
        let deadline = Instant::now() + CLIENT_DEADLINE;
        let service = scope.spawn(move || serve(server, scenario, &ready, &observed));
        let (result, peer_outcome, closed) = portal_deadline::with_socket_deadline(
            deadline,
            &AtomicBool::new(false),
            move || shutdown.shutdown(Shutdown::Both),
            move || {
                let connection = builder(client)
                    .p2p()
                    .auth_mechanism(AuthMechanism::Anonymous)
                    .max_queued(1)
                    .method_timeout(METHOD_TIMEOUT)
                    .build()
                    .map_err(portal_reply::call_error)?;
                started
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .map_err(|error| {
                        UnknownAlias::with_source(AliasUncertainty::SessionBus, error)
                    })?;
                let document = (!matches!(scenario, Scenario::Probe)).then_some("aabbccdd");
                let result = super::query_on(&connection, document, &AtomicBool::new(false));
                finished.send(()).map_err(|error| {
                    UnknownAlias::with_source(AliasUncertainty::SessionBus, error)
                })?;
                let peer_outcome = service
                    .join()
                    .unwrap_or_else(|_| Err("fake service failed".into()));
                let closed = connection.close().map_err(portal_reply::call_error);
                Ok((result, peer_outcome, closed))
            },
        )?;
        match (peer_outcome, closed) {
            (Ok(calls), Ok(())) => Ok((result, calls)),
            (peer_outcome, closed) => {
                let failure: TestError = Box::new(FixtureFailure {
                    client: result.err(),
                    service: peer_outcome.err(),
                    close: closed.err(),
                });
                Err(failure)
            }
        }
    })
}

#[test]
fn an_actual_empty_host_path_call_probes_capability_without_a_document() -> TestResult {
    let (reply, calls) = query(Scenario::Probe)?;
    let reply = reply?;
    assert_eq!(calls, 5);
    assert_eq!(reply.paths, Vec::<(String, Vec<u8>)>::new());
    assert_eq!(reply.owner_before, reply.owner_after);
    Ok(())
}

#[test]
fn a_real_fake_bus_preserves_version_owner_and_document_identity() -> TestResult {
    let (result, calls) = query(Scenario::VersionFive)?;
    let reply = result?;
    assert_eq!(
        calls, 5,
        "the owner is checked before and after the three Documents calls"
    );
    assert_eq!(
        reply.owner_before, reply.owner_after,
        "mapping is tied to one authenticated service owner"
    );
    assert_eq!(
        reply.interface, 5,
        "the actual variant property was decoded"
    );
    assert_eq!(
        reply.paths,
        [("aabbccdd".to_owned(), b"/outside/exports\0".to_vec())],
        "the real D-Bus codec preserves the exact mapping"
    );
    Ok(())
}

#[test]
fn version_four_and_denied_grants_do_not_use_another_origin_channel() -> TestResult {
    for (scenario, expected, count) in [
        (
            Scenario::VersionFour,
            AliasUncertainty::UnsupportedInterface(4),
            2,
        ),
        (Scenario::Denied, AliasUncertainty::AccessDenied, 4),
    ] {
        let (result, calls) = query(scenario)?;
        let failure = result
            .err()
            .ok_or("unsupported or denied query succeeded")?;
        assert_eq!(
            failure.reason(),
            expected,
            "unavailable host identity remains typed uncertainty"
        );
        assert_eq!(calls, count, "no xattr, Info or List fallback is attempted");
    }
    Ok(())
}

#[test]
fn a_silent_authenticated_fake_bus_hits_the_actual_method_timeout() -> TestResult {
    let (result, calls) = query(Scenario::SilentCall)?;
    assert_eq!(calls, 1, "the silent first query stops the transaction");
    assert!(
        matches!(result, Err(failure) if failure.reason() == AliasUncertainty::Timeout),
        "the blocking client enforces its configured method deadline"
    );
    Ok(())
}

#[test]
fn a_peer_closed_after_the_first_call_remains_a_session_bus_failure() -> TestResult {
    let (result, calls) = query(Scenario::ClosedCall)?;
    assert_eq!(calls, 1, "the authenticated peer observed the first method");
    assert!(
        matches!(result, Err(failure) if failure.reason() == AliasUncertainty::SessionBus),
        "an actual transport failure is not promoted to a method timeout"
    );
    Ok(())
}

#[test]
fn the_last_owner_query_detects_a_changed_service() -> TestResult {
    let (result, calls) = query(Scenario::ChangedOwner)?;
    let reply = result?;
    assert_eq!(
        calls, 5,
        "owner identity is checked again after resolving paths"
    );
    assert_ne!(
        reply.owner_before, reply.owner_after,
        "the common validator receives the service change rather than losing it"
    );
    Ok(())
}

#[test]
fn a_real_zbus_handshake_on_a_silent_socket_is_interrupted_and_joined() -> TestResult {
    let (client, mut server) = sockets()?;
    let service = thread::spawn(move || {
        let mut bytes = Vec::new();
        server.read_to_end(&mut bytes)
    });
    let shutdown = client.try_clone()?;
    let result = portal_deadline::with_socket_deadline(
        Instant::now() + Duration::from_millis(60),
        &AtomicBool::new(false),
        move || shutdown.shutdown(Shutdown::Both),
        || {
            builder(client)
                .p2p()
                .auth_mechanism(AuthMechanism::Anonymous)
                .max_queued(1)
                .build()
                .map_err(portal_reply::call_error)
        },
    );
    service.join().map_err(|_| "silent peer failed")??;
    assert!(
        matches!(result, Err(failure) if failure.reason() == AliasUncertainty::Timeout),
        "the method timeout alone does not cover an unfinished handshake"
    );
    Ok(())
}
