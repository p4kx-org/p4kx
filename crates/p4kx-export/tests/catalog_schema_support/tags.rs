// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Map, Value, json};

use super::source_values::{present, source};
use p4kx_extraction::SourceValue;

fn header(index: u32, kind: &str) -> Map<String, Value> {
    Map::from_iter([
        (
            "guid".to_owned(),
            present(json!("00000000-0000-0000-0000-000000000001")),
        ),
        (
            "recordName".to_owned(),
            present(json!(format!("{kind}.Fixture"))),
        ),
        ("className".to_owned(), present(json!("Fixture"))),
        (
            "recordPath".to_owned(),
            present(json!("synthetic/tags.xml")),
        ),
        ("recordIndex".to_owned(), present(json!(index))),
        ("sourceType".to_owned(), present(json!(kind))),
        ("fileName".to_owned(), present(json!("synthetic/tags.xml"))),
        ("metadataTag".to_owned(), source(SourceValue::Absent)),
    ])
}

pub(super) fn definitions() -> Value {
    let mut row = header(1, "Tag");
    row.extend([
        ("tagName".to_owned(), present(json!("Fixture"))),
        ("legacyGuid".to_owned(), present(json!(u32::MAX))),
        ("children".to_owned(), present(json!([]))),
    ]);
    json!({"availability":"available", "rows":[row]})
}

pub(super) fn roots() -> Value {
    let mut row = header(2, "TagDatabase");
    row.insert("tags".to_owned(), present(json!([])));
    json!({"availability":"available", "rows":[row]})
}

pub(super) fn entities() -> Value {
    let mut row = header(3, "EntityClassDefinition");
    row.insert("tags".to_owned(), present(json!([])));
    json!({"availability":"available", "rows":[row]})
}

pub(super) fn paths() -> Value {
    json!({"availability":"available", "rows":[{
        "guid": present(json!("00000000-0000-0000-0000-000000000001")),
        "recordIndex": present(json!(1)),
        "path": present(json!("Fixture")),
        "depth": present(json!(0)),
        "listed": present(json!(false)),
        "parent": present(json!({"type":"root"}))
    }]})
}
