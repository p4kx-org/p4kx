// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "linux")]
use std::time::Duration;

use serde::Serialize;
use zbus::Message;
use zbus::blocking::Connection;
use zbus::zvariant::DynamicType;

use crate::portal_alias::{AliasUncertainty, UnknownAlias};
use crate::portal_reply;
use crate::portal_resolution::{DOCUMENTS_INTERFACE_HOST_PATHS, DocumentsReply};

const DOCUMENTS: &str = "org.freedesktop.portal.Documents";
const DOCUMENTS_PATH: &str = "/org/freedesktop/portal/documents";
const BUS: &str = "org.freedesktop.DBus";
#[cfg(target_os = "linux")]
pub(crate) const METHOD_TIMEOUT: Duration = Duration::from_millis(500);
#[cfg(target_os = "linux")]
pub(crate) const QUERY_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(target_os = "linux")]
pub(crate) struct DocumentsClient<'cancel> {
    pub(crate) cancel: &'cancel AtomicBool,
}

#[cfg(target_os = "linux")]
impl crate::portal_resolution::DocumentsTransport for DocumentsClient<'_> {
    fn lookup(&mut self, document_id: &str) -> Result<DocumentsReply, UnknownAlias> {
        use std::net::Shutdown;
        use std::time::Instant;
        use zbus::blocking::connection::Builder;
        use zbus::connection::AuthMechanism;

        let deadline = Instant::now() + QUERY_TIMEOUT;
        let stream = crate::portal_socket::connect_session(deadline)?;
        let shutdown = stream
            .try_clone()
            .map_err(|error| UnknownAlias::with_source(AliasUncertainty::SessionBus, error))?;
        crate::portal_deadline::with_socket_deadline(
            deadline,
            self.cancel,
            move || shutdown.shutdown(Shutdown::Both),
            || {
                let connection = Builder::async_io_unix_stream(stream)
                    .auth_mechanism(AuthMechanism::External)
                    .max_queued(1)
                    .method_timeout(METHOD_TIMEOUT)
                    .build()
                    .map_err(portal_reply::call_error)?;
                lookup_on(&connection, document_id, self.cancel)
            },
        )
    }
}

pub(crate) fn lookup_on(
    connection: &Connection,
    document_id: &str,
    cancel: &AtomicBool,
) -> Result<DocumentsReply, UnknownAlias> {
    let owner_before =
        portal_reply::owner(&call(connection, BUS, Query::Owner, &(DOCUMENTS,), cancel)?)?;
    let interface = portal_reply::interface(
        &call(
            connection,
            &owner_before,
            Query::Version,
            &(DOCUMENTS, "version"),
            cancel,
        )?,
        &owner_before,
    )?;
    if interface < DOCUMENTS_INTERFACE_HOST_PATHS {
        return Err(UnknownAlias::new(AliasUncertainty::UnsupportedInterface(
            interface,
        )));
    }
    let mount_point = portal_reply::mount_point(
        &call(connection, &owner_before, Query::Mount, &(), cancel)?,
        &owner_before,
    )?;
    let response = call(
        connection,
        &owner_before,
        Query::Paths,
        &(vec![document_id],),
        cancel,
    )?;
    let body_bytes = response.body().len();
    let paths = portal_reply::host_paths(&response, &owner_before)?;
    let owner_after =
        portal_reply::owner(&call(connection, BUS, Query::Owner, &(DOCUMENTS,), cancel)?)?;
    Ok(DocumentsReply {
        owner_before,
        owner_after,
        interface,
        mount_point,
        body_bytes,
        paths,
    })
}

#[derive(Clone, Copy)]
enum Query {
    Owner,
    Version,
    Mount,
    Paths,
}

fn call(
    connection: &Connection,
    destination: &str,
    query: Query,
    body: &(impl Serialize + DynamicType),
    cancel: &AtomicBool,
) -> Result<Message, UnknownAlias> {
    if cancel.load(Ordering::Acquire) {
        return Err(UnknownAlias::new(AliasUncertainty::Cancelled));
    }
    let (path, interface, method) = match query {
        Query::Owner => ("/org/freedesktop/DBus", BUS, "GetNameOwner"),
        Query::Version => (DOCUMENTS_PATH, "org.freedesktop.DBus.Properties", "Get"),
        Query::Mount => (DOCUMENTS_PATH, DOCUMENTS, "GetMountPoint"),
        Query::Paths => (DOCUMENTS_PATH, DOCUMENTS, "GetHostPaths"),
    };
    connection
        .call_method(Some(destination), path, Some(interface), method, body)
        .map_err(portal_reply::call_error)
}

#[cfg(test)]
mod tests;
