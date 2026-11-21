// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::forge::{dataforge, install};
use crate::records::every_page;
use crate::support::TestResult;

#[test]
fn structs_list_their_parent_and_the_fields_they_declare() -> TestResult {
    let install = install(dataforge(0, 0))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let result = server.ok("list_structs", &json!({ "name": "*i*" }))?;
    assert_eq!(
        result["structs"],
        json!([
            {
                "name": "Item",
                "fields": [
                    { "name": "mass", "type": "Single", "array": false },
                    { "name": "size", "type": "Enum", "array": false, "enum": "Size" },
                    {
                        "name": "maker",
                        "type": "Reference",
                        "array": false,
                        "target": "Manufacturer"
                    }
                ]
            },
            {
                "name": "Ship",
                "parent": "Item",
                "fields": [
                    { "name": "crew", "type": "Int32", "array": false },
                    { "name": "tags", "type": "String", "array": true }
                ]
            }
        ]),
        "by name, with the struct or enum of each field"
    );
    assert_eq!(result["total"], 2, "the matching structs");
    Ok(())
}

#[test]
fn structs_page_by_name_ignoring_case() -> TestResult {
    let install = install(dataforge(0, 0))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let (structs, pages) = every_page(
        &mut server,
        "list_structs",
        &json!({ "limit": 3 }),
        "structs",
    )?;
    let names: Vec<&str> = structs
        .iter()
        .filter_map(|definition| definition["name"].as_str())
        .collect();
    assert_eq!(
        (names, pages),
        (vec!["Item", "Manufacturer", "Note", "Ship"], 2),
        "every struct once"
    );
    Ok(())
}

#[test]
fn enums_list_their_values_in_order() -> TestResult {
    let install = install(dataforge(0, 0))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut result = server.ok("list_enums", &json!({}))?;
    let object = result.as_object_mut().ok_or("enum result object")?;
    object.remove("fieldOrigins").ok_or("enum field origins")?;
    object.remove("provenance").ok_or("enum provenance")?;
    assert_eq!(
        result,
        json!({
            "archive": install.archive().display().to_string(),
            "total": 1,
            "enums": [{ "name": "Size", "values": ["Small", "Medium", "Large"] }]
        }),
        "the enum"
    );
    let none = server.ok("list_enums", &json!({ "name": "Colour" }))?;
    assert_eq!(
        none["enums"],
        Value::Array(Vec::new()),
        "no match is no error"
    );
    Ok(())
}
