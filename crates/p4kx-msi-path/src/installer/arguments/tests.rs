// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::argument_identity;
use crate::installer::arguments::{self, Command, Stage};
use crate::installer::caller::Goal;
use crate::installer::fault::Fault;

const PRODUCT: &str = "{00000000-0000-0000-0000-000000000001}";
const PACKAGE: &str = "{00000000-0000-0000-0000-000000000002}";
const NONCE: &str = "0101010101010101010101010101010101010101010101010101010101010101";

fn words(values: &[&str]) -> Vec<Vec<u16>> {
    values
        .iter()
        .map(|value| value.encode_utf16().collect())
        .collect()
}

fn ensure(folder: &str) -> Vec<Vec<u16>> {
    words(&[
        "prepare", "ensure", "0", PRODUCT, PACKAGE, NONCE, "", folder,
    ])
}

#[test]
fn queued_requests_preserve_spaces_unicode_and_the_exact_own_folder_separator() -> Result<(), Fault>
{
    let command = arguments::parse(ensure(r"C:\P4KX 日本語\bin\."))?;
    let Command::Transaction(invocation) = command else {
        return Err(Fault::InvalidArguments);
    };
    assert_eq!(
        invocation.stage,
        Stage::Prepare,
        "the write-ahead phase is explicit"
    );
    assert_eq!(
        invocation.caller.product, 1,
        "the MSI product is parsed as a typed identifier"
    );
    assert_eq!(
        invocation.caller.package, 2,
        "the package identifier is independent of product identity"
    );
    let Goal::Ensure(lease) = invocation.caller.goal else {
        return Err(Fault::InvalidArguments);
    };
    let expected: Vec<u8> = r"C:\P4KX 日本語\bin\"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert_eq!(
        lease.entry, expected,
        "only the explicit dot argument marker is removed"
    );
    Ok(())
}

#[test]
fn removal_has_no_authority_to_invent_a_folder_or_predecessor() -> Result<(), Fault> {
    let result = arguments::parse(words(&[
        "rollback", "remove", "12", PRODUCT, PACKAGE, NONCE,
    ]))?;
    assert!(
        matches!(result, Command::Transaction(ref request) if request.stage == Stage::Rollback && request.expected == 12 && request.caller.goal == Goal::Remove && request.caller.predecessor.is_none()),
        "remove has the exact captured caller and sequence without a guessed target"
    );
    assert_eq!(
        arguments::parse(words(&[
            "apply",
            "remove",
            "0",
            PRODUCT,
            PACKAGE,
            NONCE,
            "",
            r"C:\P4KX\."
        ])),
        Err(Fault::InvalidArguments),
        "unused removal fields are rejected"
    );
    Ok(())
}

#[test]
fn notify_is_a_distinct_command_without_registry_arguments() {
    assert_eq!(
        arguments::parse(words(&["notify"])),
        Ok(Command::Notify),
        "notification cannot smuggle a Registry key or payload"
    );
    assert_eq!(
        arguments::parse(words(&["notify", "other"])),
        Err(Fault::InvalidArguments),
        "notification is fixed rather than a generic Win32 command"
    );
}

#[test]
fn argument_work_is_bounded_before_collecting_an_unbounded_request() {
    assert_eq!(
        arguments::parse(words(&["x"; 9])),
        Err(Fault::ArgumentLimit),
        "extra fields do not grow the request indefinitely"
    );
    let oversized = vec![u16::from(b'x'); arguments::MAX_ARGUMENT_WORDS + 1];
    assert_eq!(
        arguments::parse([oversized]),
        Err(Fault::ArgumentLimit),
        "one oversized field is rejected"
    );
}

#[test]
fn relative_parent_device_and_multi_entry_folders_are_rejected() {
    for folder in [
        r"relative\.",
        r"C:relative\.",
        r"C:\..\P4KX\.",
        r"\\?\C:\P4KX\.",
        r"\\.\pipe\name\.",
        r"C:\P4KX;Other\.",
        r"C:\.",
        r"C:\P4KX\",
    ] {
        assert!(
            arguments::parse(ensure(folder)).is_err(),
            "an invalid own-folder argument cannot reach Registry mutation"
        );
    }
}

#[test]
fn network_folder_components_are_validated_without_expanding_or_accessing_them() {
    assert!(
        arguments::parse(ensure(r"\\server\share\P4KX\.")).is_ok(),
        "a complete absolute UNC spelling needs no filesystem access"
    );
    assert!(
        arguments::parse(ensure(r"\\server\..\P4KX\.")).is_err(),
        "UNC share traversal is not accepted"
    );
}

#[test]
fn code_parser_rejects_wrong_shapes_zero_and_multiple_predecessors() {
    for code in [
        "",
        "{00000000-0000-0000-0000-000000000000}",
        "00000000000000000000000000000001",
        "{00000000-0000-0000-0000-000000000001};{00000000-0000-0000-0000-000000000002}",
    ] {
        let encoded: Vec<u16> = code.encode_utf16().collect();
        assert_eq!(
            argument_identity::code(&encoded),
            Err(Fault::InvalidIdentity),
            "only one nonzero MSI GUID is an authority"
        );
    }
}

#[test]
fn sequence_parser_rejects_prefixes_aliases_and_wraparound() {
    for sequence in ["", "01", "+1", "#1", "-1", "18446744073709551616"] {
        let encoded: Vec<u16> = sequence.encode_utf16().collect();
        assert!(
            argument_identity::sequence(&encoded).is_err(),
            "a captured receipt has exactly one decimal spelling"
        );
    }
}
