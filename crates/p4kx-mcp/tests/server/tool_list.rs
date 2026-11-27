// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::support::{Install, TestResult};

#[test]
fn the_tool_list_holds_the_archive_and_dataforge_tools_all_read_only() -> TestResult {
    let install = Install::standard()?;
    let mut server = install.serve(&[])?;
    let tools = server.tools()?;
    let mut names: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "archive_info",
            "entity_loadout",
            "get_audio_excerpt",
            "get_entity_sounds",
            "get_model_info",
            "get_record",
            "get_wem_info",
            "image_preview",
            "list_channels",
            "list_chunks",
            "list_entries",
            "list_enums",
            "list_model_items",
            "list_soundbank_objects",
            "list_soundbank_wems",
            "list_structs",
            "mtl_summary",
            "read_entry_bytes",
            "read_entry_text",
            "resolve_audio_trigger",
            "run_query",
            "search_audio_triggers",
            "search_entries",
            "search_records"
        ],
        "the Archive, DataForge, model and audio tools"
    );
    Ok(())
}

#[test]
fn every_tool_is_annotated_read_only_and_declares_its_result_shape() -> TestResult {
    let install = Install::standard()?;
    let mut server = install.serve(&[])?;
    let tools = server.tools()?;
    for tool in &tools {
        let annotations = tool.get("annotations").cloned().unwrap_or_default();
        assert_eq!(
            annotations,
            json!({
                "title": annotations.get("title").cloned().unwrap_or_default(),
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }),
            "{tool} is read-only"
        );
        assert!(
            tool.get("description").and_then(Value::as_str).is_some(),
            "{tool} has a description"
        );
        assert_origin_schema(tool)?;
    }
    Ok(())
}

fn assert_origin_schema(tool: &Value) -> TestResult {
    let name = tool
        .get("name")
        .and_then(Value::as_str)
        .ok_or("tool name")?;
    let schema = tool.get("outputSchema").ok_or("tool output schema")?;
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{name} has no output properties"))?;
    let required_path = match name {
        "list_model_items" => "/allOf/0/then/required",
        _ => "/required",
    };
    let required = schema
        .pointer(required_path)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{name} has no required output fields at {required_path}"))?;
    let origin_fields: &[&str] = if name == "list_channels" {
        &["originApplicability"]
    } else {
        &["fieldOrigins", "provenance"]
    };
    for &field in origin_fields {
        assert!(properties.contains_key(field), "{name} declares {field}");
        assert!(
            required.iter().any(|entry| entry.as_str() == Some(field)),
            "{name} requires {field} in its structured result"
        );
    }
    Ok(())
}

#[test]
fn the_tools_that_return_game_content_say_it_may_be_forwarded_and_not_redistributed() -> TestResult
{
    let install = Install::standard()?;
    let mut server = install.serve(&[])?;
    let tools = server.tools()?;
    for name in [
        "read_entry_text",
        "read_entry_bytes",
        "get_record",
        "run_query",
        "get_model_info",
        "list_model_items",
        "list_entries",
        "search_entries",
        "list_chunks",
        "search_records",
        "list_structs",
        "list_enums",
        "search_audio_triggers",
        "get_entity_sounds",
        "resolve_audio_trigger",
        "list_soundbank_objects",
        "list_soundbank_wems",
        "get_wem_info",
        "get_audio_excerpt",
        "image_preview",
        "mtl_summary",
        "entity_loadout",
    ] {
        let description = tools
            .iter()
            .find(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
            .and_then(|tool| tool.get("description").and_then(Value::as_str))
            .unwrap_or_default();
        assert!(
            description.contains("forwarded by the client to third parties")
                && description.contains("must not be redistributed"),
            "{name}: {description}"
        );
    }
    Ok(())
}
