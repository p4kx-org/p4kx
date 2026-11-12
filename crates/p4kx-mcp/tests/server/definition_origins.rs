// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestEntry, TestStruct, TestType};
use serde_json::{Value, json};

use crate::forge::{dataforge, install};
use crate::model_item_origin_checks::{assert_all_addresses, assert_site, field, marker};
use crate::support::{Install, Server, TestResult};

#[test]
fn struct_values_use_original_property_and_target_definition_positions() -> TestResult {
    let install = encrypted_forge()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call("list_structs", &json!({"name":"*i*","limit":1}))?;
    let page = response.get("structuredContent").ok_or("struct page")?;
    assert_eq!(page.get("total"), Some(&json!(2)), "unchanged count");
    assert_eq!(
        page.pointer("/structs/0/name"),
        Some(&json!("Item")),
        "first sorted struct"
    );
    marker(field(page, "structs/0/fields/1/enum")?, true);
    for (namespace, label, slot) in [
        ("dataforgePropertyDefinition", "target", "2"),
        ("dataforgeEnumDefinition", "name", "0"),
    ] {
        assert_site(page, "structs/0/fields/1/enum", namespace, label, &[slot])?;
    }
    assert_site(
        page,
        "structs/0/fields/2/target",
        "dataforgeStructDefinition",
        "name",
        &["0"],
    )?;
    marker(field(page, "total")?, true);
    marker(field(page, "next_cursor")?, true);
    assert_all_addresses(&response)?;
    let cursor = page.get("next_cursor").ok_or("cursor")?;
    let next = server.ok(
        "list_structs",
        &json!({"name":"*i*","limit":1,"cursor":cursor}),
    )?;
    assert_eq!(
        next.pointer("/structs/0/name"),
        Some(&json!("Ship")),
        "second sorted struct"
    );
    assert_eq!(
        next.pointer("/structs/0/parent"),
        Some(&json!("Item")),
        "unchanged parent"
    );
    assert_site(
        &next,
        "structs/0/parent",
        "dataforgeStructDefinition",
        "parent",
        &["2"],
    )?;
    assert_site(
        &next,
        "structs/0/fields/1/array",
        "dataforgePropertyDefinition",
        "conversion",
        &["5"],
    )?;
    Ok(())
}

#[test]
fn enum_values_use_their_original_option_table_positions() -> TestResult {
    let install = encrypted_forge()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call("list_enums", &json!({}))?;
    let page = response.get("structuredContent").ok_or("enum page")?;
    assert_eq!(
        page.pointer("/enums/0/values"),
        Some(&json!(["Small", "Medium", "Large"])),
        "unchanged option order"
    );
    marker(field(page, "enums/0/values/1")?, true);
    assert_site(
        page,
        "enums/0/values/1",
        "dataforgeEnumOption",
        "name",
        &["1"],
    )?;
    assert_site(
        page,
        "enums/0/values",
        "dataforgeEnumDefinition",
        "optionRange",
        &["0", "0", "3"],
    )?;
    marker(field(page, "total")?, true);
    assert_all_addresses(&response)
}

fn encrypted_forge() -> TestResult<Install> {
    Install::new(vec![
        TestEntry::zstd("Data/Game2.dcb", dataforge(0, 0)).encrypted(),
    ])
}

#[test]
fn plain_archive_and_standalone_declarations_keep_their_own_source() -> TestResult {
    let bytes = dataforge(0, 0);
    let install = install(bytes.clone())?;
    let file = install.work().join("schema.dcb");
    std::fs::write(&file, bytes)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    for (tool, key) in [("list_structs", "structs"), ("list_enums", "enums")] {
        let archived = server.ok(tool, &json!({}))?;
        let external = server.ok(tool, &json!({"archive":file}))?;
        assert_eq!(
            archived.get(key),
            external.get(key),
            "equal definition values"
        );
        for output in [format!("{key}/0/name"), "total".to_owned()] {
            marker(field(&archived, &output)?, false);
            let origin = field(&external, &output)?;
            assert_eq!(
                origin.get("containsEncryptedSource"),
                Some(&Value::Null),
                "the standalone DCB has unknown original encryption"
            );
            assert_eq!(
                origin.get("sourceSetComplete"),
                Some(&json!(true)),
                "the actual physical file is known"
            );
        }
        assert_eq!(
            external.pointer("/provenance/sources/0/kind"),
            Some(&json!("file")),
            "the physical input is a standalone DCB"
        );
    }
    Ok(())
}

#[test]
fn differently_sorted_enum_definitions_keep_distinct_repeated_option_rows() -> TestResult {
    let data = DataForgeBuilder::new()
        .enumeration("Zulu", ["same", "same"])
        .enumeration("Alpha", ["same", "other"])
        .build();
    let install = install(data)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.ok("list_enums", &json!({"limit":1}))?;
    assert_eq!(
        first.pointer("/enums/0/name"),
        Some(&json!("Alpha")),
        "case-insensitive result order"
    );
    assert_site(
        &first,
        "enums/0/name",
        "dataforgeEnumDefinition",
        "name",
        &["1"],
    )?;
    assert_site(
        &first,
        "enums/0/values/0",
        "dataforgeEnumOption",
        "name",
        &["2"],
    )?;
    assert_site(
        &first,
        "total",
        "dataforgeEnumDefinition",
        "definitionCount",
        &[],
    )?;
    assert_site(
        &first,
        "next_cursor",
        "dataforgeEnumDefinition",
        "definitionNames",
        &["0", "2"],
    )?;
    let cursor = first.get("next_cursor").ok_or("cursor")?;
    let second = server.ok("list_enums", &json!({"limit":1,"cursor":cursor}))?;
    assert_eq!(
        second.pointer("/enums/0/values"),
        Some(&json!(["same", "same"])),
        "same option values remain separate"
    );
    for slot in ["0", "1"] {
        assert_site(
            &second,
            &format!("enums/0/values/{slot}"),
            "dataforgeEnumOption",
            "name",
            &[slot],
        )?;
    }
    Ok(())
}

#[test]
fn both_definition_lists_charge_provenance_to_the_existing_page_budget() -> TestResult {
    let count = 420;
    let data = (0..count)
        .fold(DataForgeBuilder::new(), |forge, index| {
            forge
                .structure(TestStruct::new(format!("S{index:04}")).field("value", TestType::Int32))
                .enumeration(format!("E{index:04}"), ["One", "Two", "Three"])
        })
        .build();
    let install = install(data)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    assert_budgeted_pages(&mut server, "list_structs", "structs", 'S', count)?;
    assert_budgeted_pages(&mut server, "list_enums", "enums", 'E', count)
}

fn assert_budgeted_pages(
    server: &mut Server,
    tool: &str,
    key: &str,
    prefix: char,
    count: usize,
) -> TestResult {
    let mut query = json!({"limit":1000});
    let mut seen = 0;
    let mut pages = 0;
    loop {
        let response = server.call(tool, &query)?;
        let reply = response.get("structuredContent").ok_or("definition page")?;
        let rows = reply
            .get(key)
            .and_then(Value::as_array)
            .ok_or("definition rows")?;
        assert_eq!(reply.get("total"), Some(&json!(count)), "unchanged total");
        assert!(
            serde_json::to_vec(reply)?.len() <= 256 * 1024,
            "the complete response fits"
        );
        assert_all_addresses(&response)?;
        assert!(!rows.is_empty(), "each cursor advances");
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(
                row.get("name"),
                Some(&json!(format!("{prefix}{seen:04}"))),
                "unbroken sorted sequence"
            );
            marker(field(reply, &format!("{key}/{index}/name"))?, false);
            seen += 1;
        }
        pages += 1;
        let Some(cursor) = reply.get("next_cursor") else {
            break;
        };
        query = json!({"limit":1000,"cursor":cursor});
    }
    assert!(pages > 1, "provenance makes the result page");
    assert_eq!(seen, count, "every definition appears once");
    Ok(())
}

#[test]
fn one_declaration_that_cannot_fit_is_explicitly_refused() -> TestResult {
    let structure = (0..650).fold(TestStruct::new("Wide"), |structure, index| {
        structure.field(format!("field_{index:04}"), TestType::Int32)
    });
    let data = DataForgeBuilder::new()
        .structure(structure)
        .enumeration("Long", (0..1800).map(|index| format!("Option{index:04}")))
        .build();
    let install = install(data)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    for tool in ["list_structs", "list_enums"] {
        let error = server.error(tool, &json!({}))?;
        assert!(
            error.contains("response cap"),
            "an oversized declaration is reported whole: {error}"
        );
    }
    Ok(())
}
