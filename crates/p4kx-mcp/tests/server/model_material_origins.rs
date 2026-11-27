// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestEntry, TestModel, TestMtl, TestMtlMaterial};
use serde_json::{Value, json};

use crate::model_item_origin_checks::{assert_all_addresses, field, marker, uses};
use crate::support::{Install, TestResult};

const MODEL: &str = "Data/Objects/Test/cube.cgf";

#[test]
fn material_pages_retain_only_the_returned_rows_with_original_positions() -> TestResult {
    let cube = TestModel::cube();
    let library = TestMtl::multi(&[
        TestMtlMaterial::new("first"),
        TestMtlMaterial::new("second"),
    ]);
    let install = Install::new(vec![
        TestEntry::zstd(MODEL, cube.cgf()),
        TestEntry::zstd("Data/Objects/Test/cube.cgfm", cube.cgfm()),
        TestEntry::zstd("Data/Objects/Test/cube.mtl", library.cryxml()).encrypted(),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.call(
        "list_model_items",
        &json!({"path": MODEL, "section": "materials", "limit": 1}),
    )?;
    assert_all_addresses(&first)?;
    let first = &first["structuredContent"];
    assert_eq!(first["total"], 2, "two original materials");
    assert_eq!(first["items"][0]["name"], "first", "first material");
    marker(field(first, "model")?, false);
    marker(field(first, "total")?, true);
    marker(field(first, "next_cursor")?, true);

    let second = server.call(
        "list_model_items",
        &json!({"path": MODEL, "section": "materials", "limit": 1, "cursor": first["next_cursor"]}),
    )?;
    assert_all_addresses(&second)?;
    let second = &second["structuredContent"];
    assert_eq!(second["items"][0]["id"], 1, "original material ID");
    assert_eq!(second["items"][0]["name"], "second", "second material");
    assert!(second.get("next_cursor").is_none(), "the last page");
    marker(field(second, "items/0/name")?, true);

    let first_site = name_site(first)?;
    let second_site = name_site(second)?;
    assert_ne!(
        first_site["location"]["indices"],
        second_site["location"]["indices"]
    );
    for (page, absent_site) in [(first, second_site), (second, first_site)] {
        let inventory = page["provenance"]["uses"]
            .as_array()
            .ok_or("registered uses")?;
        assert!(
            inventory
                .iter()
                .all(|site| site["location"] != absent_site["location"]),
            "other page's name origin"
        );
    }
    Ok(())
}

fn name_site(page: &Value) -> TestResult<&Value> {
    uses(page, "items/0/name")?
        .into_iter()
        .find(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str)
                == Some("materialXmlAttribute")
                && site.pointer("/location/field").and_then(Value::as_str) == Some("Name")
        })
        .ok_or_else(|| "original Name attribute was not observed".into())
}
