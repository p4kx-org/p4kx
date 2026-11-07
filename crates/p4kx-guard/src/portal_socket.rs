// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::net::UnixStream;
use std::time::Instant;

use rustix::event::{self, PollFd, PollFlags, Timespec};
use rustix::fs::{self, OFlags};
use rustix::net::{self, AddressFamily, SocketAddrUnix, SocketFlags, SocketType, sockopt};
use rustix::process;
use zbus::address::transport::UnixSocket;
use zbus::address::{Address, Transport};

use crate::portal_alias::{AliasUncertainty, UnknownAlias};
use crate::portal_reply;

pub(crate) fn connect_session(deadline: Instant) -> Result<UnixStream, UnknownAlias> {
    let address = Address::session().map_err(portal_reply::call_error)?;
    let Transport::Unix(unix) = address.transport() else {
        return Err(UnknownAlias::new(AliasUncertainty::SessionBus));
    };
    let socket_address = match unix.path() {
        UnixSocket::File(path) => SocketAddrUnix::new(path),
        UnixSocket::Abstract(name) => SocketAddrUnix::new_abstract_name(name.as_bytes()),
        UnixSocket::Dir(_) | UnixSocket::TmpDir(_) | _ => {
            return Err(UnknownAlias::new(AliasUncertainty::SessionBus));
        }
    }
    .map_err(io_failure)?;
    let descriptor = net::socket_with(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
        None,
    )
    .map_err(io_failure)?;
    match net::connect(&descriptor, &socket_address) {
        Ok(()) => {}
        Err(rustix::io::Errno::INPROGRESS | rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(UnknownAlias::new(AliasUncertainty::Timeout));
                }
                let timeout = Timespec::try_from(remaining)
                    .map_err(|error| UnknownAlias::with_source(AliasUncertainty::Limit, error))?;
                let mut pending = [PollFd::new(&descriptor, PollFlags::OUT)];
                match event::poll(&mut pending, Some(&timeout)) {
                    Ok(0) => return Err(UnknownAlias::new(AliasUncertainty::Timeout)),
                    Ok(_) => break,
                    Err(rustix::io::Errno::INTR) => {}
                    Err(error) => return Err(io_failure(error)),
                }
            }
            sockopt::socket_error(&descriptor)
                .map_err(io_failure)?
                .map_err(io_failure)?;
        }
        Err(error) => return Err(io_failure(error)),
    }
    if sockopt::socket_peercred(&descriptor)
        .map_err(io_failure)?
        .uid
        != process::geteuid()
    {
        return Err(UnknownAlias::new(AliasUncertainty::SessionBus));
    }
    let flags = fs::fcntl_getfl(&descriptor).map_err(io_failure)?;
    fs::fcntl_setfl(&descriptor, flags & !OFlags::NONBLOCK).map_err(io_failure)?;
    Ok(UnixStream::from(descriptor))
}

fn io_failure(error: rustix::io::Errno) -> UnknownAlias {
    let reason = if error == rustix::io::Errno::TIMEDOUT {
        AliasUncertainty::Timeout
    } else {
        AliasUncertainty::SessionBus
    };
    UnknownAlias::with_source(reason, error)
}
