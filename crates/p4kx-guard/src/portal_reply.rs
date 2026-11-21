// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use zbus::message::Type;
use zbus::zvariant::OwnedValue;
use zbus::{Message, names};

use crate::portal_alias::{AliasUncertainty, UnknownAlias};
use crate::portal_path::MAX_PATH_BYTES;
use crate::portal_resolution::MAX_REPLY_BYTES;

#[cfg(test)]
mod tests;

pub(crate) fn checked_reply(
    message: &Message,
    sender: &str,
) -> Result<zbus::message::Body, UnknownAlias> {
    if message.body().len() > MAX_REPLY_BYTES {
        return Err(UnknownAlias::new(AliasUncertainty::Limit));
    }
    let header = message.header();
    if message.message_type() != Type::MethodReturn
        || header.sender().map(names::UniqueName::as_str) != Some(sender)
    {
        return Err(UnknownAlias::new(AliasUncertainty::MalformedReply));
    }
    Ok(message.body())
}

pub(crate) fn owner(message: &Message) -> Result<String, UnknownAlias> {
    let body = checked_reply(message, "org.freedesktop.DBus")?;
    let name: String = body.deserialize().map_err(|_| malformed())?;
    names::UniqueName::try_from(name.as_str()).map_err(|_| malformed())?;
    Ok(name)
}

pub(crate) fn interface(message: &Message, sender: &str) -> Result<u32, UnknownAlias> {
    let body = checked_reply(message, sender)?;
    let value: OwnedValue = body.deserialize().map_err(|_| malformed())?;
    u32::try_from(value).map_err(|_| malformed())
}

pub(crate) fn mount_point(message: &Message, sender: &str) -> Result<Vec<u8>, UnknownAlias> {
    let body = checked_reply(message, sender)?;
    let path: BytePath = body.deserialize().map_err(|_| malformed())?;
    Ok(path.bytes)
}

pub(crate) fn host_paths(
    message: &Message,
    sender: &str,
) -> Result<Vec<(String, Vec<u8>)>, UnknownAlias> {
    let body = checked_reply(message, sender)?;
    let paths: HostPaths = body.deserialize().map_err(|_| malformed())?;
    Ok(paths.entries)
}

pub(crate) fn call_error(error: zbus::Error) -> UnknownAlias {
    if let zbus::Error::MethodError(name, _, message) = &error {
        if message.body().len() > MAX_REPLY_BYTES {
            return UnknownAlias::new(AliasUncertainty::Limit);
        }
        let reason = match name.as_str() {
            "org.freedesktop.DBus.Error.AccessDenied" | "org.freedesktop.DBus.Error.AuthFailed" => {
                AliasUncertainty::AccessDenied
            }
            "org.freedesktop.DBus.Error.NoReply"
            | "org.freedesktop.DBus.Error.Timeout"
            | "org.freedesktop.DBus.Error.TimedOut" => AliasUncertainty::Timeout,
            "org.freedesktop.DBus.Error.NameHasNoOwner"
            | "org.freedesktop.DBus.Error.ServiceUnknown" => AliasUncertainty::ServiceUnavailable,
            "org.freedesktop.DBus.Error.UnknownMethod" => AliasUncertainty::UnsupportedMethod,
            _ => AliasUncertainty::MalformedReply,
        };
        return UnknownAlias::new(reason);
    }
    match error {
        zbus::Error::InputOutput(source) | zbus::Error::Connection(source, _) => {
            let reason = if source.kind() == std::io::ErrorKind::TimedOut {
                AliasUncertainty::Timeout
            } else {
                AliasUncertainty::SessionBus
            };
            UnknownAlias::with_source(reason, source)
        }
        zbus::Error::ExcessData => UnknownAlias::new(AliasUncertainty::Limit),
        zbus::Error::FDO(source) => UnknownAlias::new(match *source {
            zbus::fdo::Error::AccessDenied(_) | zbus::fdo::Error::AuthFailed(_) => {
                AliasUncertainty::AccessDenied
            }
            zbus::fdo::Error::NoReply(_)
            | zbus::fdo::Error::Timeout(_)
            | zbus::fdo::Error::TimedOut(_) => AliasUncertainty::Timeout,
            zbus::fdo::Error::UnknownMethod(_) => AliasUncertainty::UnsupportedMethod,
            zbus::fdo::Error::NameHasNoOwner(_) | zbus::fdo::Error::ServiceUnknown(_) => {
                AliasUncertainty::ServiceUnavailable
            }
            _ => AliasUncertainty::SessionBus,
        }),
        _ => UnknownAlias::new(AliasUncertainty::SessionBus),
    }
}

const fn malformed() -> UnknownAlias {
    UnknownAlias::new(AliasUncertainty::MalformedReply)
}

#[derive(zbus::zvariant::Type)]
#[zvariant(signature = "ay")]
struct BytePath {
    bytes: Vec<u8>,
}

impl<'data> Deserialize<'data> for BytePath {
    fn deserialize<Decode: Deserializer<'data>>(
        deserializer: Decode,
    ) -> Result<Self, Decode::Error> {
        struct BytesVisitor;
        impl<'data> Visitor<'data> for BytesVisitor {
            type Value = BytePath;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded Unix bytestring")
            }
            fn visit_seq<Sequence: SeqAccess<'data>>(
                self,
                mut sequence: Sequence,
            ) -> Result<Self::Value, Sequence::Error> {
                use serde::de::Error as _;
                let mut bytes = Vec::new();
                while let Some(byte) = sequence.next_element()? {
                    if bytes.len() > MAX_PATH_BYTES {
                        return Err(Sequence::Error::custom("path limit exceeded"));
                    }
                    bytes.push(byte);
                }
                Ok(BytePath { bytes })
            }
        }
        deserializer.deserialize_seq(BytesVisitor)
    }
}

#[derive(zbus::zvariant::Type)]
#[zvariant(signature = "a{say}")]
struct HostPaths {
    entries: Vec<(String, Vec<u8>)>,
}

impl<'data> Deserialize<'data> for HostPaths {
    fn deserialize<Decode: Deserializer<'data>>(
        deserializer: Decode,
    ) -> Result<Self, Decode::Error> {
        struct PathsVisitor;
        impl<'data> Visitor<'data> for PathsVisitor {
            type Value = HostPaths;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("one document host-path mapping")
            }
            fn visit_map<Entries: MapAccess<'data>>(
                self,
                mut map: Entries,
            ) -> Result<Self::Value, Entries::Error> {
                use serde::de::Error as _;
                let mut entries = Vec::new();
                while let Some((id, path)) = map.next_entry::<String, BytePath>()? {
                    if !entries.is_empty() {
                        return Err(Entries::Error::custom("ambiguous document mapping"));
                    }
                    entries.push((id, path.bytes));
                }
                Ok(HostPaths { entries })
            }
        }
        deserializer.deserialize_map(PathsVisitor)
    }
}
