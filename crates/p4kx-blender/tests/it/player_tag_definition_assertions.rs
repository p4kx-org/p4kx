// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_assembly::BodyVariant;
use p4kx_blender::{PlayerDocument, PlayerDocumentSink, PlayerSource, PlayerSourceCapture};
use p4kx_extraction::CancelToken;
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::TestResult;

#[derive(Default)]
struct Captured(Vec<Value>);

impl PlayerDocumentSink for Captured {
    fn document(&mut self, original: PlayerDocument<'_>) -> io::Result<()> {
        self.0.push(serde_json::to_value(original.source)?);
        Ok(())
    }
}

fn entries() -> [TestEntry; 4] {
    let root = br#"<TagDefinition version="2"><Imports>
        <Import filename="Animations/test/scoped-tags.xml"/>
        <Import filename="DATA/ANIMATIONS/TEST/SCOPED-TAGS.XML"/>
        <Import filename="Animations/test/missing.xml"/>
        <Import filename="Animations/test/unrecognized.xml"/>
        <Import filename="Animations/test/unreadable.xml"/>
        <Unknown filename="Animations/test/should-not-lookup.xml"/>
        <Import/>
    </Imports><Tags><Tag name="Plain" priority="0"/><Group name="movement">
        <Tag name="stationary" priority="1"/><Tag name="a&amp;b"/>
    </Group><Tag name="AfterGroup"/></Tags></TagDefinition>"#;
    let scoped = br#"<TagDefinition version="2"><Imports>
        <Import filename="Animations/test/root-tags.xml"/>
    </Imports><Tags><Tag name="Scoped"/></Tags></TagDefinition>"#;
    [
        TestEntry::stored("Data/Animations/test/root-tags.xml", root),
        TestEntry::stored("Data/Animations/test/scoped-tags.xml", scoped).encrypted(),
        TestEntry::stored("Data/Animations/test/unrecognized.xml", br"<UnknownRoot/>"),
        TestEntry::stored("Data/Animations/test/unreadable.xml", br"<TagDefinition/>")
            .with_crc32c(0),
    ]
}

#[test]
fn tag_definitions_preserve_authored_scopes_import_occurrences_and_cycles() -> TestResult {
    let (_folder, archive) =
        super::fixture_with(super::subadb_graph_assertions::ROOT_LINKS, entries())?;
    let cancel = CancelToken::new();
    let body = BodyVariant::Male;
    let legacy = serde_json::to_value(PlayerSource::read(&archive, body, &cancel)?)?;
    let mut documents = Captured::default();
    let data = serde_json::to_value(PlayerSourceCapture::read(
        &archive,
        &[body],
        &cancel,
        &mut documents,
    )?)?;
    assert_eq!(
        legacy.get("motion"),
        data.get("motion"),
        "each original v1 ID remains"
    );
    super::shared_capture::same_legacy_inventory(&legacy, &data)?;
    let graph = data.get("tagDefinitions").ok_or("tag topology")?;
    let roots = graph
        .get("roots")
        .and_then(Value::as_array)
        .ok_or("authored roots")?;
    let controller_global = roots
        .iter()
        .find(|row| row["role"] == "controllerTags")
        .ok_or("controller scope")?;
    let adb_global = roots
        .iter()
        .find(|row| row["role"] == "databaseTags")
        .ok_or("ADB scope")?;
    assert_ne!(
        controller_global["owner"], adb_global["owner"],
        "independently owned selectors"
    );
    assert_eq!(
        controller_global.pointer("/target/value/data"),
        adb_global.pointer("/target/value/data"),
        "same physical global target"
    );
    verify_documents(graph, &documents)?;
    let cycles = graph
        .get("cycleBackEdges")
        .and_then(Value::as_array)
        .ok_or("back edges")?;
    let [cycle] = cycles.as_slice() else {
        return Err("two repeated forward edges share one back edge".into());
    };
    assert_eq!(
        cycle.get("owner"),
        graph.pointer("/documents/2/document"),
        "cycle closes at the scoped definition",
    );
    assert_eq!(
        cycle.get("link"),
        Some(&json!(0)),
        "closing original import"
    );
    Ok(())
}

fn verify_documents(graph: &Value, sink: &Captured) -> TestResult {
    let documents = graph
        .get("documents")
        .and_then(Value::as_array)
        .ok_or("observed definitions")?;
    assert_eq!(
        documents.len(),
        3,
        "one global, one fragment and one scoped physical definition"
    );
    for document in documents {
        let source = document.get("source").ok_or("physical source")?;
        assert_eq!(
            sink.0.iter().filter(|id| *id == source).count(),
            1,
            "one original byte admission"
        );
    }
    let [global, fragments, _scoped] = documents.as_slice() else {
        return Err("three physical documents".into());
    };
    verify_groups(global);
    verify_imports(global, fragments)
}

fn verify_groups(global: &Value) {
    assert_eq!(
        global.pointer("/declaration/sections/0/members/0/type"),
        Some(&json!("tag")),
        "loose tag precedes authored group",
    );
    assert_eq!(
        global.pointer("/declaration/sections/0/members/1/tags/1/name/value/value"),
        Some(&json!("a&b")),
        "group member preserves authored text",
    );
    assert_eq!(
        global.pointer("/declaration/sections/0/members/2/type"),
        Some(&json!("tag")),
        "loose tag follows authored group",
    );
}

fn verify_imports(global: &Value, fragments: &Value) -> TestResult {
    let imports = global
        .get("links")
        .and_then(Value::as_array)
        .ok_or("authored imports")?;
    let [first, alias, missing, invalid, unreadable, omitted] = imports.as_slice() else {
        return Err("each actual Import kept without inferring unknown element role".into());
    };
    assert_eq!(
        first.pointer("/target/value"),
        alias.pointer("/target/value"),
        "aliases converge"
    );
    assert_eq!(
        first.pointer("/selector/value/containsEncryptedSource"),
        Some(&json!(false)),
        "plain owner"
    );
    assert_eq!(
        first.pointer("/target/containsEncryptedSource"),
        Some(&json!(true)),
        "actual encrypted target"
    );
    assert!(
        first
            .pointer("/selector/occurrences/0/position/start")
            .is_some(),
        "original selector byte position"
    );
    for (link, state) in [
        (missing, "missing"),
        (invalid, "invalidXml"),
        (unreadable, "unreadable"),
    ] {
        assert_eq!(
            link.pointer("/target/value/type"),
            Some(&json!(state)),
            "explicit disposition"
        );
    }
    assert_eq!(
        omitted.pointer("/selector/value/state"),
        Some(&json!("absent")),
        "absent filename"
    );
    assert_eq!(
        fragments.pointer("/links/0/role"),
        Some(&json!("fragmentTags")),
        "authored per-fragment edge"
    );
    assert_eq!(
        fragments.pointer("/links/0/target/value/data"),
        first.pointer("/target/value/data"),
        "scoped target shares physical definition"
    );
    Ok(())
}
