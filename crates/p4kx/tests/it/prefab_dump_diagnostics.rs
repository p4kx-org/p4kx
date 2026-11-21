// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original input facts retained by unreadable Prefab diagnostics.

use std::error::Error;

use p4kx_testkit::{ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use serde_json::Value;

use super::{TestResult, entry_path, field, original_bytes, original_uses, run};

pub(super) fn assert_text_root(report: &Value) -> TestResult {
    let reason = field(report, "/issues/0/reason")?;
    assert_eq!(
        report.pointer("/issues/0/kind").and_then(Value::as_str),
        Some("library_unreadable"),
        "the rejected library yields the expected kind"
    );
    assert_eq!(
        reason["containsEncryptedSource"], true,
        "the printed root name came from encrypted XML"
    );
    assert_eq!(
        reason["sourceSetComplete"], true,
        "the parser retained the exact rejected root position"
    );
    assert_eq!(
        field(report, "/issues/0/kind")?["containsEncryptedSource"],
        false,
        "the issue category contains no decrypted value"
    );
    let original = root_site(report, reason, "broken.xml")?;
    assert_eq!(
        original_bytes("<NotALibrary/>", original)?,
        "<NotALibrary/>",
        "the original position identifies the rejected XML element"
    );
    Ok(())
}

fn root_site<'a>(
    report: &'a Value,
    reason: &Value,
    filename: &str,
) -> Result<&'a Value, Box<dyn Error>> {
    original_uses(report, reason)?
        .into_iter()
        .find(|usage| {
            usage["role"] == "payload"
                && entry_path(report, usage).is_ok_and(|path| path.ends_with(filename))
        })
        .ok_or_else(|| "missing retained root name".into())
}

fn malformed_fixture() -> Result<Value, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let binary = TestElement::new("RejectedBinaryRoot")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let root = concat!(
        "<PrefabsLibrary><Prefab Name='Start'><Objects>",
        "<Object Type='Prefab' Name='binary' PrefabName='test/binary.Bad'/>",
        "<Object Type='Prefab' Name='repaired' PrefabName='test/repaired.Bad'/>",
        "<Object Type='Prefab' Name='malformedText' PrefabName='test/text.Bad'/>",
        "<Object Type='Prefab' Name='malformedBinary' PrefabName='test/truncated.Bad'/>",
        "</Objects></Prefab></PrefabsLibrary>",
    );
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Prefabs/test/root.xml",
            root.as_bytes(),
        ))
        .entry(TestEntry::stored("Data/Prefabs/test/binary.xml", binary).encrypted())
        .entry(
            TestEntry::stored(
                "Data/Prefabs/test/repaired.xml",
                b"<RepairedRoot why='\xFF'/>",
            )
            .encrypted(),
        )
        .entry(
            TestEntry::stored(
                "Data/Prefabs/test/text.xml",
                b"<PrefabsLibrary><PrivateName></WrongName></PrefabsLibrary>",
            )
            .encrypted(),
        )
        .entry(TestEntry::stored("Data/Prefabs/test/truncated.xml", b"CryXmlB\0").encrypted())
        .write_to(&path)?;
    let output = run(&["dump", "test/root", "Start"], &path, true)?;
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    Ok(serde_json::from_str(&output.stdout)?)
}

#[test]
fn decrypted_prefab_diagnostics_distinguish_retained_and_discarded_parser_values() -> TestResult {
    let report = malformed_fixture()?;
    let binary = field(&report, "/issues/0/reason")?;
    assert_eq!(binary["containsEncryptedSource"], true);
    assert_eq!(binary["sourceSetComplete"], true);
    let use_site = root_site(&report, binary, "binary.xml")?;
    assert_eq!(
        use_site
            .pointer("/location/namespace")
            .and_then(Value::as_str),
        Some("cryxmlNode")
    );
    assert_eq!(
        use_site
            .pointer("/location/indices/0")
            .and_then(Value::as_str),
        Some("0")
    );

    let repaired = field(&report, "/issues/2/reason")?;
    assert_eq!(repaired["containsEncryptedSource"], true);
    assert_eq!(repaired["sourceSetComplete"], false);
    let use_site = root_site(&report, repaired, "repaired.xml")?;
    assert_eq!(
        use_site.pointer("/location/type").and_then(Value::as_str),
        Some("opaque")
    );

    for (index, fixed_message) in [
        (4, "the file is not well-formed XML"),
        (6, "the file is not valid CryXml"),
    ] {
        let path = format!("/issues/{index}/reason");
        let value = report
            .pointer(&path)
            .and_then(Value::as_str)
            .ok_or("unreadable reason")?;
        assert!(value.ends_with(fixed_message), "{value}");
        let origin = field(&report, &path)?;
        assert_eq!(origin["containsEncryptedSource"], false, "{path}");
        assert_eq!(origin["sourceSetComplete"], true, "{path}");
    }
    Ok(())
}
