// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use predicates::prelude::predicate;
use serde_json::Value;

use super::TestResult;

#[test]
fn key_write_json_carries_metadata_applicability() -> TestResult {
    use predicates::prelude::PredicateBooleanExt;

    let dir = tempfile::tempdir()?;
    let hex = p4kx_testkit::test_key().to_hex();
    let stored = super::keyless(dir.path())?
        .args(["--json", "key", "set", &hex, "--archive"])
        .arg(super::archive(dir.path())?)
        .assert()
        .code(0)
        .stdout(predicate::str::contains(&hex).not());
    let stored: Value = serde_json::from_slice(&stored.get_output().stdout)?;
    assert_eq!(stored["action"], "stored", "{stored}");
    assert_eq!(stored["originApplicability"], "notApplicable", "{stored}");
    for action in ["removed", "none_stored"] {
        let removal = super::keyless(dir.path())?
            .args(["--json", "key", "remove"])
            .assert()
            .code(0);
        let removal: Value = serde_json::from_slice(&removal.get_output().stdout)?;
        assert_eq!(removal["action"], action, "{removal}");
        assert_eq!(removal["originApplicability"], "notApplicable", "{removal}");
    }
    Ok(())
}
