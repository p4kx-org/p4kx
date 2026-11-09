// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Complete selections page normally; unreadable conditions stay tool errors on repeated calls.

use p4kx_testkit::query_selection_sample;
use serde_json::json;

use crate::forge;
use crate::support::TestResult;

#[test]
fn a_late_witness_is_returned_and_a_normal_unknown_is_a_complete_empty_result() -> TestResult {
    let install = forge::install(query_selection_sample::database().build())?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let arguments = json!({"query":"from Station select id where tags[*] = 4096", "limit":1});
    for _ in 0..2 {
        let result = server.ok("run_query", &arguments)?;
        assert_eq!(result["total"], 1, "the late witness is included");
        assert_eq!(
            result["rows"],
            json!([{"record path":"synthetic/station","id":7}]),
            "the complete row survives cached reuse"
        );
        assert!(
            result["next_cursor"].is_null(),
            "the one-row result has no next page"
        );
    }
    let result = server.ok(
        "run_query",
        &json!({"query":"from Station where owner.ready = true"}),
    )?;
    assert_eq!(result["total"], 0, "normal unknown selects nothing");
    assert_eq!(
        result["rows"],
        json!([]),
        "normal unknown is a successful empty result"
    );
    Ok(())
}

#[test]
fn a_failed_selection_cannot_be_cached_as_an_empty_success() -> TestResult {
    let install = forge::install(query_selection_sample::database().build())?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let arguments = json!({"query":"from Station where ready = true"});
    for _ in 0..2 {
        let error = server.error("run_query", &arguments)?;
        assert!(
            error.contains("synthetic/station")
                && error.contains("comparison 1")
                && error.contains("bool byte 2"),
            "{error}"
        );
    }
    let result = server.ok(
        "run_query",
        &json!({"query":"from Station where ready = true or id = 7"}),
    )?;
    assert_eq!(
        result["total"], 1,
        "a decisive proof succeeds after failed calls"
    );
    Ok(())
}
