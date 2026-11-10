// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::exact_loadout_assertions as check;
use crate::exact_loadout_fixture::{self as fixture, Texts};
use crate::exact_loadout_source;
use crate::support::TestResult;

#[test]
fn exact_loadout_strings_preserve_all_output_identity_and_definition_values() -> TestResult {
    let texts = Texts::long();
    let install = fixture::install(&texts)?;
    let node = exact_loadout_source::original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let output = server.ok("entity_loadout", &json!({"entity":node.guid().to_string()}))?;
    check::exact(&output, &node, &texts)?;
    check::roundtrip(&mut server, &output, &node)
}

#[test]
fn exact_loadout_returned_identity_above_input_cap_is_explicitly_refused() -> TestResult {
    let mut texts = Texts::short();
    texts.class = format!("Long_{}", "é".repeat(600));
    let install = fixture::install(&texts)?;
    let node = exact_loadout_source::original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let output = server.ok("entity_loadout", &json!({"entity":node.guid().to_string()}))?;
    assert_eq!(
        output.pointer("/entity/class_name"),
        Some(&json!(texts.class)),
        "complete value above input cap"
    );
    let returned = output
        .pointer("/entity/class_name")
        .and_then(Value::as_str)
        .ok_or("class")?;
    let error = server.error("entity_loadout", &json!({"entity":returned}))?;
    assert!(
        error.contains("entity") && error.contains("longer than 1024 bytes"),
        "explicit transport limit: {error}"
    );
    Ok(())
}

#[test]
fn exact_loadout_oversized_envelope_uses_response_admission() -> TestResult {
    let mut texts = Texts::short();
    texts.class = "R".repeat(300 << 10);
    let install = fixture::install(&texts)?;
    let node = exact_loadout_source::original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    check::too_large(&mut server, &json!({"entity":node.guid().to_string()}))
}

#[test]
fn exact_loadout_aggregate_item_strings_use_one_response_budget() -> TestResult {
    let mut texts = Texts::short();
    texts.display = format!("@{}", "d".repeat(100 << 10));
    texts.flags = "f".repeat(100 << 10);
    texts.helper = "h".repeat(100 << 10);
    let install = fixture::install(&texts)?;
    let node = exact_loadout_source::original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    check::too_large(&mut server, &json!({"entity":node.guid().to_string()}))
}

#[test]
fn exact_loadout_late_formatted_error_preserves_prefix_and_retry_cursor() -> TestResult {
    let mut texts = Texts::short();
    texts.missing = "M".repeat(300 << 10);
    let install = fixture::install(&texts)?;
    let node = exact_loadout_source::original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let arguments = json!({"entity":node.guid().to_string()});
    let first = server.ok("entity_loadout", &arguments)?;
    assert_eq!(
        first.get("total"),
        Some(&json!(2)),
        "full original total remains"
    );
    let ports = first
        .get("ports")
        .and_then(Value::as_array)
        .ok_or("page ports")?;
    assert_eq!(
        ports.len(),
        1,
        "keep the complete prefix before the enormous formatted error"
    );
    assert_eq!(
        ports.first().and_then(|port| port.get("name")),
        Some(&json!(texts.port)),
        "the original first item"
    );
    let cursor = first
        .get("next_cursor")
        .and_then(Value::as_str)
        .ok_or("next cursor")?;
    check::too_large(
        &mut server,
        &json!({"entity":node.guid().to_string(),"cursor":cursor}),
    )
}

#[test]
fn exact_loadout_unshown_type_and_later_item_are_not_charged() -> TestResult {
    let mut texts = Texts::short();
    texts.types = (0..16)
        .map(|index| (format!("Type{index}"), Vec::new()))
        .collect();
    let huge = "H".repeat(300 << 10);
    texts.types.push((huge.clone(), vec![huge.clone()]));
    texts.missing = huge.clone();
    let install = fixture::install(&texts)?;
    let node = exact_loadout_source::original(&install, &texts)?;
    let definition = node
        .ports()
        .first()
        .and_then(|port| port.definition())
        .ok_or("original definition")?;
    let hidden = definition
        .types()
        .get(16)
        .ok_or("actual seventeenth type")?;
    assert_eq!(hidden.name(), huge, "huge unshown original name exists");
    assert_eq!(
        hidden.sub_types(),
        [huge],
        "huge unshown original subtype exists"
    );
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.ok(
        "entity_loadout",
        &json!({"entity":node.guid().to_string(),"limit":1}),
    )?;
    assert_eq!(
        first.pointer("/ports/0/definition/type_count"),
        Some(&json!(17)),
        "honest original preview count"
    );
    assert_eq!(
        first
            .pointer("/ports/0/definition/types")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(16),
        "this slice preserves the current preview contract"
    );
    assert_eq!(
        first.get("ports").and_then(Value::as_array).map(Vec::len),
        Some(1),
        "later page item was not materialized"
    );
    assert!(
        first.get("next_cursor").and_then(Value::as_str).is_some(),
        "later item remains reachable"
    );
    Ok(())
}
