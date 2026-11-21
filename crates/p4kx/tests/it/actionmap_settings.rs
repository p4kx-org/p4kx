// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestEntry;
use serde_json::Value;

use super::{TestResult, archive};

#[test]
fn settings_cli_publishes_json_for_complete_and_partial_profiles() -> TestResult {
    const GENERAL: &str = "Data/Libs/Config/Profiles/default/attributes.xml";
    const VR: &str = "Data/Libs/Config/Profiles/default/attributesVR.xml";
    const VALID: &str = "<Attributes><Attr name='fixture_setting' value='001.50'/></Attributes>";

    let directory = tempfile::tempdir()?;
    for (bytes, code, expected) in [(VALID, 0, "read"), ("not XML", 1, "malformed")] {
        let path = archive(
            directory.path(),
            vec![
                TestEntry::stored(GENERAL, bytes),
                TestEntry::stored(VR, VALID),
            ],
        )?;
        let output = p4kx_testkit::p4kx()?
            .args([
                "--key",
                &p4kx_testkit::test_key().to_hex(),
                "actionmap",
                "settings",
            ])
            .arg(path)
            .output()?;
        assert_eq!(
            output.status.code(),
            Some(code),
            "profile result: {output:?}"
        );
        let report: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(
            report
                .pointer("/settings/documents/0/status/value")
                .and_then(Value::as_str),
            Some(expected),
            "the CLI preserves the General decision even on partial failure"
        );
        assert_eq!(
            report.pointer("/settings/documents/1/elements/1/declarations/1/value/value/value"),
            Some(&Value::from("001.50")),
            "the unambiguous VR setting is present on both exits"
        );
        assert!(
            report
                .pointer("/provenance/uses")
                .is_some_and(Value::is_array),
            "the CLI includes the same-document origin inventory"
        );
    }
    Ok(())
}
