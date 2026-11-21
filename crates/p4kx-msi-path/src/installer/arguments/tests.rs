// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::argument_identity;
use crate::installer::arguments::{self, Command, Stage};
use crate::installer::binding::Binding;
use crate::installer::caller::Goal;
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;

const PRODUCT: &str = "{00000000-0000-0000-0000-000000000001}";
const PACKAGE: &str = "{00000000-0000-0000-0000-000000000002}";

fn words(values: &[&str]) -> Vec<Vec<u16>> {
    values
        .iter()
        .map(|value| value.encode_utf16().collect())
        .collect()
}

fn captured() -> String {
    format!("v1|0|{:032x}|{:032x}|{}", 1, 2, "01".repeat(32))
}

fn ensure(folder: &str) -> Vec<Vec<u16>> {
    words(&["seal", "ensure", PRODUCT, PACKAGE, &captured(), "", folder])
}

#[test]
fn reservations_preserve_spaces_unicode_and_the_exact_own_folder_separator() -> Result<(), Fault> {
    let command = arguments::parse(ensure(r"C:\P4KX 日本語\bin\."))?;
    let Command::Seal { declaration, .. } = command else {
        return Err(Fault::InvalidArguments);
    };
    assert_eq!(
        declaration.product, 1,
        "the MSI product is parsed as a typed identifier"
    );
    assert_eq!(
        declaration.package, 2,
        "the package identifier is independent of product identity"
    );
    let Goal::Ensure(lease) = declaration.goal else {
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
    let result = arguments::parse(words(&["seal", "remove", PRODUCT, PACKAGE, &captured()]))?;
    assert!(
        matches!(result, Command::Seal { ref declaration, .. } if declaration.goal == Goal::Remove && declaration.predecessor.is_none()),
        "removal seals only its typed product/package without a folder"
    );
    let binding = Binding {
        sequence: 12,
        product: 1,
        package: 2,
        nonce: InvocationNonce::from_bytes([1; 32])?,
    }
    .encode();
    let result = arguments::parse(words(&["rollback", PRODUCT, PACKAGE, &binding]))?;
    assert!(
        matches!(result, Command::Transaction(ref request) if request.stage == Stage::Rollback && request.binding.sequence == 12),
        "rollback carries the exact captured binding without a guessed target"
    );
    assert_eq!(
        arguments::parse(words(&[
            "apply",
            "remove",
            "0",
            PRODUCT,
            PACKAGE,
            "nonce",
            "",
            r"C:\P4KX\."
        ])),
        Err(Fault::ArgumentLimit),
        "the old unbound goal/nonce transport is not accepted"
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

#[test]
fn bound_callbacks_reject_another_packages_identity_and_missing_transport() -> Result<(), Fault> {
    let binding = Binding {
        sequence: 0,
        product: 1,
        package: 2,
        nonce: InvocationNonce::from_bytes([1; 32])?,
    }
    .encode();
    for head in [
        "prepare",
        "apply",
        "commit",
        "rollback",
        "cancel-reserved",
        "recover-reserved",
    ] {
        assert!(
            arguments::parse(words(&[head, PRODUCT, PACKAGE, &binding])).is_ok(),
            "every phase uses the same canonical captured transport"
        );
        assert_eq!(
            arguments::parse(words(&[head, PACKAGE, PACKAGE, &binding])),
            Err(Fault::InvalidIdentity),
            "a foreign MSI product cannot adopt a binding"
        );
        assert!(
            arguments::parse(words(&[head, PRODUCT, PACKAGE, "unavailable"])).is_err(),
            "AppSearch miss/sentinel cannot authorise a callback"
        );
    }
    Ok(())
}

#[test]
fn early_reserve_accepts_identity_only_and_rejects_the_unsealed_legacy_goal_abi()
-> Result<(), Fault> {
    assert_eq!(
        arguments::parse(words(&["reserve", PRODUCT, PACKAGE]))?,
        Command::Reserve {
            product: 1,
            package: 2
        },
        "early reservation has no costing-dependent input"
    );
    for values in [
        words(&["reserve", "remove", PRODUCT, PACKAGE]),
        words(&["reserve", "ensure", PRODUCT, PACKAGE, "", r"C:\P4KX\."]),
    ] {
        assert!(
            arguments::parse(values).is_err(),
            "a goal cannot enter the pre-costing command"
        );
    }
    assert_eq!(
        arguments::parse(words(&["seal", "remove", PACKAGE, PACKAGE, &captured()])),
        Err(Fault::InvalidIdentity),
        "seal cannot substitute the captured product"
    );
    assert!(
        arguments::parse(words(&["seal", "remove", PRODUCT, PACKAGE, "unbound"])).is_err(),
        "a search miss cannot seal"
    );
    Ok(())
}
