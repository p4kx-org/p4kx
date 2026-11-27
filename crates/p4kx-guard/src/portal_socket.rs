// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::os::fd::AsFd;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::net::UnixStream;
use std::time::Instant;

use nix::sys::socket::{self, UnixCredentials};
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
    authenticate_peer(peer_credentials(&descriptor)?)?;

    let flags = fs::fcntl_getfl(&descriptor).map_err(io_failure)?;
    fs::fcntl_setfl(&descriptor, flags & !OFlags::NONBLOCK).map_err(io_failure)?;
    Ok(UnixStream::from(descriptor))
}

fn peer_credentials(descriptor: &impl AsFd) -> Result<UnixCredentials, UnknownAlias> {
    socket::getsockopt(descriptor, socket::sockopt::PeerCredentials)
        .map_err(|error| UnknownAlias::with_source(AliasUncertainty::SessionBus, error))
}

fn authenticate_peer(credentials: UnixCredentials) -> Result<(), UnknownAlias> {
    if credentials.uid() != process::geteuid().as_raw() {
        return Err(UnknownAlias::new(AliasUncertainty::SessionBus));
    }
    Ok(())
}

fn io_failure(error: rustix::io::Errno) -> UnknownAlias {
    let reason = if error == rustix::io::Errno::TIMEDOUT {
        AliasUncertainty::Timeout
    } else {
        AliasUncertainty::SessionBus
    };
    UnknownAlias::with_source(reason, error)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::os::unix::net::{UnixDatagram, UnixStream};

    use nix::libc::ucred;
    use nix::sys::socket::UnixCredentials;
    use rustix::process;

    use crate::portal_alias::AliasUncertainty;

    #[test]
    fn unbound_peer_credentials_preserve_zero_pid_and_unknown_uid() -> Result<(), Box<dyn Error>> {
        let socket = UnixDatagram::unbound()?;
        let credentials = super::peer_credentials(&socket)?;
        assert_eq!(
            credentials.pid(),
            0,
            "An unbound socket has no visible peer PID"
        );
        assert_eq!(
            credentials.uid(),
            u32::MAX,
            "Unknown UID remains a credential value"
        );
        assert_eq!(
            credentials.gid(),
            u32::MAX,
            "Unknown GID remains a credential value"
        );
        assert!(
            matches!(super::authenticate_peer(credentials), Err(error) if error.reason() == AliasUncertainty::SessionBus),
            "An unknown peer UID must not authenticate"
        );
        Ok(())
    }

    #[test]
    fn connected_peers_authenticate_the_actual_owner_uid() -> Result<(), Box<dyn Error>> {
        let streams: [UnixStream; 2] = UnixStream::pair()?.into();
        for stream in streams {
            let credentials = super::peer_credentials(&stream)?;
            assert_eq!(
                credentials.pid(),
                process::getpid().as_raw_nonzero().get(),
                "The connected pair retains the actual peer PID"
            );
            assert_eq!(
                credentials.uid(),
                process::geteuid().as_raw(),
                "The connected pair retains the actual peer UID"
            );
            super::authenticate_peer(credentials)?;
        }
        Ok(())
    }

    #[test]
    fn zero_pid_does_not_override_authenticated_uid() -> Result<(), Box<dyn Error>> {
        let credentials = UnixCredentials::from(ucred {
            pid: 0,
            uid: process::geteuid().as_raw(),
            gid: process::getegid().as_raw(),
        });
        super::authenticate_peer(credentials)?;
        Ok(())
    }
}
