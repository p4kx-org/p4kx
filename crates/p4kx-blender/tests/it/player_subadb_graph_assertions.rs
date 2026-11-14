// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestEntry;
use serde_json::Value;

use super::TestResult;

pub(super) const ROOT_LINKS: &str = r#"<SubADBs>
    <SubADB File="Animations/test/branch.adb" Tags="Mode+Upper"/>
    <SubADB File="DATA\Animations\test\BRANCH.ADB" Tags="Mode+Lower"/>
    <SubADB File="Animations/test/player.adb"/>
    <SubADB File="Animations/test/missing.adb"/>
    <SubADB File="Animations/test/duplicate.adb"/>
    <SubADB File="Animations/test/invalid.adb"/>
    <SubADB File="Animations/test/locked.adb"/>
    <SubADB File="../private.adb"/>
    <SubADB/>
</SubADBs>"#;

pub(super) fn entries() -> [TestEntry; 5] {
    let branch =
        br#"<AnimDB TagDef="Animations/test/tags.xml" FragDef="Animations/test/fragments.xml">
        <SubADBs><SubADB File="Animations/test/player.adb" Tags="Return"/></SubADBs>
        <FragmentList><Walk><Fragment Tags="branch" FragTags="forward"/></Walk></FragmentList>
    </AnimDB>"#;
    [
        TestEntry::stored("Data/Animations/test/branch.adb", branch).encrypted(),
        TestEntry::stored("Data/Animations/test/duplicate.adb", br"<AnimDB/>"),
        TestEntry::stored(r"DATA\Animations\test\DUPLICATE.ADB", br"<AnimDB/>"),
        TestEntry::stored("Data/Animations/test/invalid.adb", br"<DifferentRoot/>"),
        TestEntry::stored("Data/Animations/test/locked.adb", br"<AnimDB/>")
            .encrypted()
            .with_crc32c(0),
    ]
}

pub(super) fn verify(document: &Value) -> TestResult {
    let graph = document
        .pointer("/motion/components/value/0/databaseGraph")
        .ok_or("database graph")?;
    let branches = graph
        .get("branches")
        .and_then(Value::as_array)
        .ok_or("physical branches")?;
    let [root, child] = branches.as_slice() else {
        return Err("expected root and one reused child".into());
    };
    assert_eq!(
        graph.pointer("/root/value"),
        root.pointer("/document/value"),
        "root document identity"
    );
    verify_root(root, child)?;
    assert_eq!(
        child
            .pointer("/links/0/traversal/value")
            .and_then(Value::as_str),
        Some("cycle"),
        "cross-document back edge is identified",
    );
    assert_eq!(
        child
            .pointer("/links/0/traversal/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "closing cycle uses both physically observed documents",
    );
    assert_eq!(
        child
            .pointer("/tagDefinition/value")
            .and_then(Value::as_str),
        Some("Animations/test/tags.xml"),
        "exact branch TagDef is retained",
    );
    assert_eq!(
        child
            .pointer("/options/0/fragmentTags/value")
            .and_then(Value::as_str),
        Some("forward"),
        "child options retain independent tag selectors",
    );
    Ok(())
}

fn verify_root(root: &Value, child: &Value) -> TestResult {
    let links = root
        .get("links")
        .and_then(Value::as_array)
        .ok_or("root links")?;
    let [
        first,
        repeated,
        own,
        missing,
        ambiguous,
        invalid_xml,
        locked,
        unsafe_path,
        absent,
    ] = links.as_slice()
    else {
        return Err("expected all authored link occurrences in order".into());
    };
    verify_reachable(first, repeated, own, child);
    verify_unresolved(missing, ambiguous, invalid_xml, locked, unsafe_path, absent);
    Ok(())
}

fn verify_reachable(first: &Value, repeated: &Value, own: &Value, child: &Value) {
    for (value, tags) in [(first, "Mode+Upper"), (repeated, "Mode+Lower")] {
        assert_eq!(
            value.pointer("/target/value/data"),
            child.pointer("/document/value"),
            "same physical child"
        );
        assert_eq!(
            value.pointer("/tags/value").and_then(Value::as_str),
            Some(tags),
            "branch-specific raw tag values"
        );
        assert_eq!(
            value
                .pointer("/tags/containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(false),
            "tag uses only its authored source"
        );
        assert_eq!(
            value
                .pointer("/target/containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(true),
            "link uses selected encrypted bytes"
        );
    }
    assert_eq!(
        repeated.pointer("/traversal/value").and_then(Value::as_str),
        Some("reachable"),
        "a completed shared target is not a cycle"
    );
    assert_eq!(
        own.pointer("/traversal/value").and_then(Value::as_str),
        Some("cycle"),
        "active self reference"
    );
}

fn verify_unresolved(
    missing: &Value,
    ambiguous: &Value,
    invalid_xml: &Value,
    locked: &Value,
    unsafe_path: &Value,
    absent: &Value,
) {
    let status = |value: &Value| {
        value
            .pointer("/target/value/type")
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    for (link, expected) in [
        (missing, "missing"),
        (ambiguous, "ambiguous"),
        (invalid_xml, "invalidXml"),
        (locked, "unreadable"),
        (unsafe_path, "invalidSelector"),
        (absent, "invalidSelector"),
    ] {
        assert_eq!(
            status(link).as_deref(),
            Some(expected),
            "explicit unresolved status"
        );
    }
    assert_eq!(
        locked
            .pointer("/target/sourceSetComplete")
            .and_then(Value::as_bool),
        Some(false),
        "failed byte read remains incomplete"
    );
    assert_eq!(
        locked.pointer("/target/containsEncryptedSource"),
        Some(&Value::Null),
        "attempted encrypted target does not taint original data"
    );
    assert_eq!(
        absent.pointer("/file/state").and_then(Value::as_str),
        Some("absent"),
        "omitted selector state"
    );
    assert_eq!(
        ambiguous
            .pointer("/target/value/candidates")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "both physical candidates"
    );
}
