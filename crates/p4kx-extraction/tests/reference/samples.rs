// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Synthetic data for the mutation self-tests: a Record P4KX builds from a synthetic DataForge,
//! and the JSON a reference tool would write for it.

#![expect(
    clippy::expect_used,
    reason = "the helpers build a synthetic DataForge; a failure fails the test"
)]

use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Map, Number, Value, json};

use super::starbreaker_json::{
    POINTS_TO_PREFIX, RECORD_ID_KEY, RECORD_NAME_KEY, RECORD_PATH_KEY, RECORD_TAG_KEY,
    RECORD_VALUE_KEY, TYPE_KEY,
};
use super::value_tree::{JsonNumber, Node, RecordDoc};

pub(crate) const SAMPLE_GUID: &str = "cbee34fd-6441-4000-b99f-924f7f26e19f";
const SAMPLE: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;
pub(crate) const OTHER_GUID: &str = "50e8a9f6-f956-4000-8ace-7c0ca9a91f5d";
const OTHER: u128 = 0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d;
pub(crate) const BIG: u64 = (1 << 60) + 1;

fn point(x: f32) -> TestValue {
    TestValue::Struct(TestInstance::new("Inner").value("x", TestValue::Single(x)))
}

pub(crate) fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .enumeration("Size", ["Small", "Large"])
        .structure(TestStruct::new("Inner").field("x", TestType::Single))
        .structure(
            TestStruct::new("Sample")
                .field("flag", TestType::Bool)
                .field("count", TestType::Int32)
                .field("big", TestType::UInt64)
                .field("ratio", TestType::Single)
                .field("precise", TestType::Double)
                .field("zero", TestType::Single)
                .field("label", TestType::String)
                .field("id", TestType::Guid)
                .field("size", TestType::Enum("Size".to_owned()))
                .field("inner", TestType::Class("Inner".to_owned()))
                .array("tags", TestType::String)
                .array("points", TestType::Class("Inner".to_owned()))
                .field("link", TestType::Reference),
        )
        .record(
            TestRecord::new(SAMPLE, "Sample", "Sample.One", "samples/one.xml")
                .tag("SystemsDesign")
                .value("flag", TestValue::Bool(true))
                .value("count", TestValue::Int32(-42))
                .value("big", TestValue::UInt64(BIG))
                .value("ratio", TestValue::Single(0.1))
                .value("precise", TestValue::Double(0.1))
                .value("zero", TestValue::Single(0.0))
                .value("label", TestValue::text("Hello"))
                .value("id", TestValue::Guid(OTHER))
                .value("size", TestValue::text("Large"))
                .value("inner", point(2.5))
                .value(
                    "tags",
                    TestValue::Array(vec![TestValue::text("a"), TestValue::text("b")]),
                )
                .value("points", TestValue::Array(vec![point(1.0), point(2.0)]))
                .value("link", TestValue::Reference(OTHER)),
        )
        .record(TestRecord::new(
            OTHER,
            "Sample",
            "Sample.Other",
            "samples/other.xml",
        ))
        .build()
}

/// The Record P4KX builds from the synthetic DataForge for a GUID; `None` when there is none.
///
/// # Panics
///
/// Panics when the synthetic DataForge does not open, which is a bug of the testkit.
#[must_use]
pub(crate) fn resolve(guid: &str) -> Option<RecordDoc> {
    let bytes = database();
    let forge = DataForge::open(&bytes).expect("the synthetic DataForge opens");
    let record = forge.record_by_guid(guid.parse().ok()?)?;
    Some(RecordDoc::from_tree(&record.tree().ok()?))
}

/// The Record P4KX builds from the synthetic DataForge.
///
/// # Panics
///
/// Panics when the synthetic Record does not build, which is a bug of the testkit.
#[must_use]
pub(crate) fn ours() -> RecordDoc {
    resolve(SAMPLE_GUID).expect("the synthetic Record builds")
}

fn number(node: &Node) -> Value {
    match node {
        Node::Int(number) => i64::try_from(*number).map_or_else(
            |_| json!(u64::try_from(*number).unwrap_or_default()),
            |number| json!(number),
        ),
        Node::Single(number) => json!(f64::from(*number)),
        Node::Double(number) => json!(number),
        Node::Number(JsonNumber::Signed(number)) => json!(number),
        Node::Number(JsonNumber::Unsigned(number)) => json!(number),
        Node::Number(JsonNumber::Float(number)) => {
            Number::from_f64(*number).map_or(Value::Null, Value::Number)
        }
        _ => Value::Null,
    }
}

fn value(node: &Node) -> Value {
    match node {
        Node::Null | Node::Dangling => Value::Null,
        Node::Bool(flag) => Value::Bool(*flag),
        Node::Int(_) | Node::Single(_) | Node::Double(_) | Node::Number(_) => number(node),
        Node::Text(text) | Node::Guid(text) => Value::String(text.clone()),
        Node::Reference { guid, path, .. } => json!({
            RECORD_PATH_KEY: format!("file://./../{path}.json"),
            RECORD_NAME_KEY: "Target",
            RECORD_ID_KEY: guid,
        }),
        Node::WeakPointer => json!(format!("{POINTS_TO_PREFIX}ptr:1")),
        Node::Struct { type_name, fields } => {
            let mut map = Map::new();
            map.insert(TYPE_KEY.to_owned(), Value::String(type_name.clone()));
            for (name, field) in fields {
                map.insert(name.clone(), value(field));
            }
            Value::Object(map)
        }
        Node::Array(items) => Value::Array(items.iter().map(value).collect()),
        Node::InvalidBool => Value::Bool(true),
        Node::Error(message) | Node::Unmapped(message) => json!({ "error": message }),
    }
}

/// The JSON a reference tool writes in place of a link to `doc`: the content of the Record
/// behind its GUID and name.
#[must_use]
pub(crate) fn embedded_json(doc: &RecordDoc) -> Value {
    let mut content = value(&doc.root);
    if let Value::Object(map) = &mut content {
        map.insert(RECORD_ID_KEY.to_owned(), json!(doc.guid));
        map.insert(RECORD_NAME_KEY.to_owned(), json!(doc.name));
    }
    content
}

/// The JSON a reference tool writes for `doc`: single precision widened to a double.
#[must_use]
pub(crate) fn reference_json(doc: &RecordDoc) -> Value {
    let mut map = Map::new();
    map.insert(RECORD_NAME_KEY.to_owned(), json!(doc.name));
    map.insert(RECORD_ID_KEY.to_owned(), json!(doc.guid));
    if let Some(tag) = &doc.tag {
        map.insert(RECORD_TAG_KEY.to_owned(), json!(tag));
    }
    map.insert(RECORD_VALUE_KEY.to_owned(), value(&doc.root));
    Value::Object(map)
}
