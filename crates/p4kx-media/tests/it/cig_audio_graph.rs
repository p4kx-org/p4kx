// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry, test_key,
};
use serde_json::{Value, json};

use crate::support;
use crate::support::audio_provider::{Origins, TestResult};

#[path = "cig_audio_role_origins.rs"]
mod role_origins;

const GRAPH_ENTRY: &str = "Data/Libs/CIGAudio/CIGAudioData.bin";
const DEFINITION: &str = "audio-control-cig-audio-definition";
const FIELD: &str = "audio-control-cig-audio-field";
const TARGET_NODE: &str = "00000000-0000-0000-0000-000000000002";
const PARAMETER: &str = "00000000-0000-0000-0000-000000000003";

#[test]
fn cig_audio_cryxml_preserves_graph_order_fields_and_encrypted_origins() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive_path = scratch.path().join("Data.p4k");
    let input = fixture();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(GRAPH_ENTRY, input.as_slice()).encrypted())
        .entry(TestEntry::stored(
            "Data/Libs/CIGAudio/Other/CIGAudioData.bin",
            input.as_slice(),
        ))
        .write_to(&archive_path)?;
    let archive = Archive::open_with(archive_path, OpenOptions::new().key(Some(test_key())))?;
    let output = scratch.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AudioControl],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "audio-control.jsonl")?;
    check_order(&rows)?;
    check_fields(&rows)?;
    let base = output.join(&index.artifact_base);
    let origins = Origins::read(&base)?;
    check_link_origin(&rows, &origins)?;
    role_origins::check(&rows, &origins)?;
    let (_, document) = row(&rows, "audio-control-document", &[])?;
    let raw = document
        .source
        .raw
        .as_ref()
        .ok_or("retained CryXml source")?;
    assert_eq!(
        std::fs::read(base.join(&raw.path))?,
        input,
        "the raw receipt identifies the exact binary source"
    );
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        assert!(
            validator.is_valid(&serde_json::to_value(row)?),
            "the actual graph row matches the typed contract"
        );
    }
    Ok(())
}

#[test]
fn unexpected_cig_audio_root_is_a_typed_source_failure() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive_path = scratch.path().join("Data.p4k");
    let input =
        TestElement::new("ATLConfig").to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    ArchiveBuilder::new()
        .entry(TestEntry::stored(GRAPH_ENTRY, input.as_slice()))
        .write_to(&archive_path)?;
    let archive = Archive::open(archive_path)?;
    let output = scratch.path().join("out");
    let options = ExportOptions {
        diagnostic: true,
        kinds: vec![MediaKind::AudioControl],
        ..ExportOptions::default()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("unexpected CIG audio syntax must fail the selected projection".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic output root")?;
    let rows = support::records(base, &index, "audio-control.jsonl")?;
    let [failure] = rows.as_slice() else {
        return Err("only one CIG audio source failure is expected".into());
    };
    assert_eq!(
        failure.subresource.namespace, "audio-control-failure",
        "unrecognised syntax is not a valid graph row"
    );
    assert_eq!(
        failure.data.pointer("/payload"),
        Some(&json!("unexpected-cig-audio-root")),
        "the opening element failure is machine-readable"
    );
    Ok(())
}

fn check_order(rows: &[MediaRecord]) -> TestResult {
    let definitions: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == DEFINITION)
        .map(|row| {
            (
                row.subresource.indices.as_slice(),
                row.data.pointer("/payload/role").and_then(Value::as_str),
            )
        })
        .collect();
    let expected: [&str; 16] = [
        "graph",
        "objects",
        "object",
        "parameters",
        "parameter-reference",
        "logic",
        "nodes",
        "logic-node",
        "input-links",
        "input-link",
        "links",
        "link",
        "output-links",
        "output-link",
        "logic-node",
        "object",
    ];
    assert_eq!(
        definitions.len(),
        expected.len(),
        "every known source position contributes one role row"
    );
    for (ordinal, ((indices, role), expected)) in definitions.into_iter().zip(expected).enumerate()
    {
        assert_eq!(
            indices,
            [u64::try_from(ordinal)?],
            "physical traversal order"
        );
        assert_eq!(
            role,
            Some(expected),
            "role is derived in its physical parent"
        );
    }
    check_link_element(rows)?;
    assert!(
        rows.iter().all(|row| {
            matches!(&row.source.identity, Identity::Entry { entry, .. } if entry.path == GRAPH_ENTRY)
                && !row.subresource.namespace.starts_with("audio-control-reference")
        }),
        "only the exact graph source is interpreted and no ATL or Wwise edge is inferred"
    );
    Ok(())
}

fn check_link_element(rows: &[MediaRecord]) -> TestResult {
    let (_, link) = row(rows, "audio-control-element", &[11])?;
    assert_eq!(
        link.data.pointer("/payload/qualified_name"),
        Some(&json!("0")),
        "a numeric CryXml tag keeps its original spelling"
    );
    assert_eq!(
        link.data.pointer("/payload/location/parent"),
        Some(&json!("10")),
        "link retains its owning list occurrence"
    );
    Ok(())
}

fn check_fields(rows: &[MediaRecord]) -> TestResult {
    for (indices, field, value) in [
        ([2, 0], "type", json!({"type":"text","value":"context"})),
        (
            [2, 2],
            "game-defined",
            json!({"type":"boolean","value":true}),
        ),
        (
            [2, 3],
            "duration-min",
            json!({"type":"number","value":1.25}),
        ),
        (
            [4, 0],
            "reference",
            json!({"type":"guid-token","value":PARAMETER}),
        ),
        (
            [11, 0],
            "node",
            json!({"type":"guid-token","value":TARGET_NODE}),
        ),
        (
            [11, 1],
            "source-link-id",
            json!({"type":"unsigned-integer","value":0}),
        ),
        (
            [11, 2],
            "target-link-id",
            json!({"type":"invalid-unsigned-integer"}),
        ),
    ] {
        let (_, row) = row(rows, FIELD, &indices)?;
        assert_eq!(
            row.data.pointer("/payload"),
            Some(&json!({"field":field,"typed_value":value})),
            "the typed field is attached to its exact declaration"
        );
    }
    for (indices, raw) in [([2, 3], "001.25"), ([11, 2], "-7"), ([11, 3], "keep")] {
        let (_, declaration) = row(rows, "audio-control-declaration", &indices)?;
        assert_eq!(
            declaration.data.pointer("/payload/value"),
            Some(&json!(raw)),
            "the original spelling remains available"
        );
    }
    assert!(
        row(rows, FIELD, &[11, 3]).is_err(),
        "unknown declarations are retained as original syntax"
    );
    Ok(())
}

fn check_link_origin(rows: &[MediaRecord], origins: &Origins) -> TestResult {
    let (ordinal, _) = row(rows, FIELD, &[11, 1])?;
    let receipt = origins.field_in(
        "records",
        Some("audio-control"),
        ordinal,
        "/data/payload/typed_value/value",
        true,
    )?;
    let refs = receipt
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("the link field has source-use references")?;
    let [reference] = refs.as_slice() else {
        return Err("one exact original link field is expected".into());
    };
    let proof = origins.physical_use(reference)?;
    assert_eq!(
        proof.observation.pointer("/location"),
        Some(&json!({
            "type":"subresource","namespace":"cig-audio-cryxml-attribute",
            "indices":["11","8"],"field":"declaration"
        })),
        "the proof points to the original CryXml node and absolute attribute index"
    );
    assert_eq!(
        proof.source.pointer("/path/value"),
        Some(&json!(GRAPH_ENTRY)),
        "the field proof retains its original Archive Entry"
    );
    Ok(())
}

fn row<'a>(
    rows: &'a [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'a MediaRecord)> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| {
            row.subresource.namespace == namespace && row.subresource.indices == indices
        })
        .ok_or_else(|| format!("missing {namespace} at {indices:?}").into())
}

fn fixture() -> Vec<u8> {
    let link = TestElement::new("0")
        .attribute("node", TARGET_NODE)
        .attribute("sourceLinkId", "0")
        .attribute("targetLinkId", "-7")
        .attribute("futureSetting", "keep");
    let nodes = TestElement::new("nodes")
        .child(
            TestElement::new("_3")
                .attribute("type", "logicNodeDef")
                .attribute("guid", "00000000-0000-0000-0000-000000000001")
                .child(
                    TestElement::new("inLinks")
                        .child(TestElement::new("0").child(TestElement::new("links").child(link))),
                )
                .child(TestElement::new("outLinks").child(TestElement::new("0"))),
        )
        .child(
            TestElement::new("_4")
                .attribute("type", "logicNodeDef")
                .attribute("guid", TARGET_NODE),
        );
    let context = TestElement::new("_7")
        .attribute("type", "context")
        .attribute("guid", "00000000-0000-0000-0000-000000000004")
        .attribute("gameDefined", "1")
        .attribute("durationMin", "001.25")
        .child(
            TestElement::new("parameters").child(TestElement::new("_").attribute("ref", PARAMETER)),
        )
        .child(TestElement::new("logic").child(nodes));
    let source = TestElement::new("_")
        .attribute("type", "parameter")
        .attribute("guid", PARAMETER);
    TestElement::new("CIGAudioData")
        .child(TestElement::new("objects").child(context).child(source))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}
