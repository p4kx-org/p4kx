// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;
use std::io::Read;
use std::net::Shutdown;
#[cfg(not(unix))]
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::{Duration, Instant};

use zbus::blocking::MessageIterator;
use zbus::blocking::connection::Builder;
use zbus::connection::AuthMechanism;
use zbus::zvariant::OwnedValue;
use zbus::{Guid, Message};

use crate::portal_alias::{AliasUncertainty, UnknownAlias};
use crate::{portal_deadline, portal_reply};

type TestError = Box<dyn Error + Send + Sync>;
type TestResult = Result<(), TestError>;

#[derive(Clone, Copy)]
enum Scenario {
    VersionFive,
    VersionFour,
    Denied,
    SilentCall,
    ChangedOwner,
}

#[cfg(unix)]
type TestSocket = std::os::unix::net::UnixStream;
#[cfg(not(unix))]
type TestSocket = TcpStream;

#[cfg(unix)]
fn sockets() -> std::io::Result<(TestSocket, TestSocket)> {
    TestSocket::pair()
}

#[cfg(not(unix))]
fn sockets() -> std::io::Result<(TestSocket, TestSocket)> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let client = TcpStream::connect(listener.local_addr()?)?;
    let (server, _) = listener.accept()?;
    Ok((client, server))
}

fn builder(stream: TestSocket) -> Builder<'static> {
    #[cfg(unix)]
    {
        Builder::async_io_unix_stream(stream)
    }
    #[cfg(not(unix))]
    {
        Builder::async_io_tcp_stream(stream)
    }
}

fn serve(stream: TestSocket, scenario: Scenario) -> Result<usize, TestError> {
    let stop = stream.try_clone()?;
    let result = portal_deadline::with_socket_deadline(
        Instant::now() + Duration::from_secs(3),
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
            for index in 0..5 {
                let request = incoming
                    .next()
                    .ok_or_else(|| UnknownAlias::new(AliasUncertainty::SessionBus))?
                    .map_err(portal_reply::call_error)?;
                if matches!(scenario, Scenario::SilentCall) {
                    thread::sleep(Duration::from_millis(350));
                    return Ok(index + 1);
                }
                let response =
                    response(&request, scenario, index).map_err(portal_reply::call_error)?;
                connection
                    .send(&response)
                    .map_err(portal_reply::call_error)?;
                if matches!(scenario, Scenario::VersionFour) && index == 1
                    || matches!(scenario, Scenario::Denied) && index == 3
                {
                    return Ok(index + 1);
                }
            }
            Ok(5)
        },
    );
    Ok(result?)
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
        Some("Get") => reply.build(&OwnedValue::from(
            if matches!(scenario, Scenario::VersionFour) {
                4_u32
            } else {
                5
            },
        )),
        Some("GetMountPoint") => reply.build(&b"/run/user/1001/doc\0".to_vec()),
        Some("GetHostPaths") => {
            let requested: Vec<String> = request.body().deserialize()?;
            assert_eq!(
                requested,
                ["aabbccdd"],
                "the real encoded request contains only the selected document"
            );
            if matches!(scenario, Scenario::Denied) {
                Message::error(&header, "org.freedesktop.DBus.Error.AccessDenied")?
                    .sender(sender)?
                    .build(&"private /home/not-a-real-user/document")
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
    let peer_lifetime = (client.try_clone()?, server.try_clone()?);
    let service = thread::spawn(move || serve(server, scenario));
    let shutdown = client.try_clone()?;
    let result = portal_deadline::with_socket_deadline(
        Instant::now() + Duration::from_secs(2),
        &AtomicBool::new(false),
        move || shutdown.shutdown(Shutdown::Both),
        || {
            let connection = builder(client)
                .p2p()
                .auth_mechanism(AuthMechanism::Anonymous)
                .max_queued(1)
                .method_timeout(Duration::from_millis(200))
                .build()
                .map_err(portal_reply::call_error)?;
            super::lookup_on(&connection, "aabbccdd", &AtomicBool::new(false))
        },
    );
    let calls = service.join().map_err(|_| "fake service failed")??;
    drop(peer_lifetime);
    Ok((result, calls))
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
