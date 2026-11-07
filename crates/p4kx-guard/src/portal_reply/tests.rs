// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};
use zbus::Message;
use zbus::zvariant::{DynamicType, OwnedValue};

use crate::portal_alias::AliasUncertainty;

type TestResult = Result<(), Box<dyn Error>>;

fn reply(body: &(impl Serialize + DynamicType), sender: &str) -> zbus::Result<Message> {
    let request =
        Message::method_call("/org/freedesktop/portal/documents", "GetHostPaths")?.build(&())?;
    Message::method_return(&request.header())?
        .sender(sender)?
        .build(body)
}

#[test]
fn official_reply_signatures_preserve_non_utf8_paths() -> TestResult {
    let owner = reply(&":1.7", "org.freedesktop.DBus")?;
    assert_eq!(
        super::owner(&owner)?,
        ":1.7",
        "the bus supplies a validated unique owner"
    );
    let version = reply(&OwnedValue::from(5_u32), ":1.7")?;
    assert_eq!(
        super::interface(&version, ":1.7")?,
        5,
        "version is a variant containing uint32"
    );
    let mount = reply(&b"/run/user/1001/doc\0".to_vec(), ":1.7")?;
    assert_eq!(
        super::mount_point(&mount, ":1.7")?,
        b"/run/user/1001/doc\0",
        "mount paths retain their bytestring terminator"
    );
    let path = b"/home/user/exports-\xff\0".to_vec();
    let response = reply(&BTreeMap::from([("abcd", path.clone())]), ":1.7")?;
    assert_eq!(
        super::host_paths(&response, ":1.7")?,
        [("abcd".to_owned(), path)],
        "Unix path bytes are not converted lossily to Unicode"
    );
    Ok(())
}

#[derive(zbus::zvariant::Type)]
#[zvariant(signature = "a{say}")]
struct DuplicatePaths;

impl Serialize for DuplicatePaths {
    fn serialize<Encode: Serializer>(
        &self,
        serializer: Encode,
    ) -> Result<Encode::Ok, Encode::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("abcd", &b"/first\0".to_vec())?;
        map.serialize_entry("abcd", &b"/second\0".to_vec())?;
        map.end()
    }
}

#[test]
fn duplicate_dictionary_keys_and_foreign_senders_are_not_authenticated() -> TestResult {
    let duplicate = reply(&DuplicatePaths, ":1.7")?;
    assert!(
        super::host_paths(&duplicate, ":1.7").is_err(),
        "a map decoder must not silently overwrite repeated document IDs"
    );
    let response = reply(&BTreeMap::from([("abcd", b"/outside\0".to_vec())]), ":1.8")?;
    assert!(
        super::host_paths(&response, ":1.7").is_err(),
        "a different unique service owner cannot authenticate a path"
    );
    Ok(())
}

#[test]
fn oversized_and_wrong_signature_bodies_fail_before_projection() -> TestResult {
    let huge = reply(&vec![0_u8; super::MAX_REPLY_BYTES + 1], ":1.7")?;
    let failure = super::host_paths(&huge, ":1.7")
        .err()
        .ok_or("large reply was accepted")?;
    assert_eq!(
        failure.reason(),
        AliasUncertainty::Limit,
        "the byte cap applies before interpreting the invalid dictionary signature"
    );
    let wrong = reply(&"a string is not a host-path dictionary", ":1.7")?;
    assert!(
        super::host_paths(&wrong, ":1.7").is_err(),
        "the official map signature is required"
    );
    Ok(())
}

#[test]
fn remote_method_errors_do_not_format_private_paths() -> TestResult {
    let request =
        Message::method_call("/org/freedesktop/portal/documents", "GetHostPaths")?.build(&())?;
    for (name, expected) in [
        (
            "org.freedesktop.DBus.Error.AccessDenied",
            AliasUncertainty::AccessDenied,
        ),
        (
            "org.freedesktop.DBus.Error.UnknownMethod",
            AliasUncertainty::UnsupportedMethod,
        ),
        (
            "org.freedesktop.DBus.Error.NoReply",
            AliasUncertainty::Timeout,
        ),
    ] {
        let message = Message::error(&request.header(), name)?
            .sender(":1.7")?
            .build(&"private /home/secret-user/source")?;
        let failure = super::call_error(message.into());
        assert_eq!(
            failure.reason(),
            expected,
            "remote failures retain their typed classification"
        );
        assert!(
            !format!("{failure:?} {failure}").contains("secret-user"),
            "remote message bodies must not enter diagnostic formatting"
        );
    }
    Ok(())
}
