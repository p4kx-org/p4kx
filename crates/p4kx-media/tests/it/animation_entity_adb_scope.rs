// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::{Archive, Entry, Exclusion, Filter, OpenOptions};
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestLoadout,
    TestLoadoutEntry, TestPort, TestStruct, TestType, TestValue, entity_schema, test_key,
};
use serde_json::{Value, json};

use super::animation_definition_origins::Origins;
use super::support::{self, TestResult};

const STREAM: &str = "animation-definition.jsonl";
const ENTITY: &str = "FixtureMannequinOwner";
const PART: &str = "FixtureMannequinAttachment";
const DCB: &str = "Data/Game2.dcb";
const ROOT: &str = "Data/Animations/Scope/root.adb";
const CHILD: &str = "Data/Animations/Scope/child.adb";
const ATTACHMENT: &str = "Data/Animations/Scope/attachment.adb";
const TAGS: &str = "Data/Animations/Scope/tags.xml";
const IMPORTED_TAGS: &str = "Data/Animations/Scope/imported-tags.xml";
const EXCLUDED: &str = "Data/Animations/Scope/excluded.adb";
const UNRELATED: &str = "Data/Animations/Scope/unrelated.adb";
const ANIMATED: &str = "SAnimatedCharacterComponentParams";
const CONTROLLER: &str = "SAnimationControllerParams";
const ROOT_SELECTOR: &str = r" ANIMATIONS\Scope\Root.ADB ";
const DUPLICATE_SELECTOR: &str = "Data/Animations/Scope/ROOT.adb";
const ATTACHMENT_SELECTOR: &str = "Animations/Scope/attachment.adb";

const ROOT_XML: &[u8] = br#"<AnimDB TagDef="Animations/Scope/tags.xml"><SubADBs>
  <SubADB File="ANIMATIONS\Scope\Child.ADB" Tags="One"/>
  <SubADB File="Data/Animations/Scope/child.adb" Tags="Two"/>
  <SubADB File="Animations/Scope/missing.adb"/>
  <SubADB File="Animations/Scope/excluded.adb"/>
  <SubADB File="Animations/Scope/ambiguous.adb"/>
</SubADBs></AnimDB>"#;
const CHILD_XML: &[u8] = br#"<AnimDB><SubADBs>
  <SubADB File="Animations/Scope/root.adb"/>
</SubADBs></AnimDB>"#;
const ATTACHMENT_XML: &[u8] = br#"<AnimDB><FragmentList><Idle><Fragment>
  <AnimLayer><Animation name="attachmentClip"/></AnimLayer>
</Fragment></Idle></FragmentList></AnimDB>"#;
const TAGS_XML: &[u8] = br#"<TagDefinition><Imports>
  <Import filename="Data\Animations\Scope\IMPORTED-TAGS.XML"/>
</Imports><Tags><Tag name="Two"/></Tags></TagDefinition>"#;
const IMPORTED_XML: &[u8] = br#"<TagDefinition><Tags><Tag name="ONE"/></Tags></TagDefinition>"#;

#[test]
fn entity_adb_scope_keeps_authored_placements_and_transitive_sub_database_edges() -> TestResult {
    let temp = tempfile::tempdir()?;
    let archive = fixture(temp.path())?;
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        animation_entity: Some(ENTITY.into()),
        include: Some(Filter::parse(UNRELATED)?),
        exclude: Exclusion::parse([EXCLUDED])?,
        ..ExportOptions::default()
    };
    let output = temp.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let [kind] = index.kinds.as_slice() else {
        return Err("one selected provider stream".into());
    };
    assert_eq!(
        kind.provider.kind,
        MediaKind::AnimationDefinition,
        "graph metadata exports without the Animation provider"
    );
    let rows = support::records(&output, &index, STREAM)?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    let origins = Origins::read(&output.join(index.artifact_base))?;
    verify_documents(&rows);
    verify_bindings(&archive, &rows, &origins)?;
    verify_graph(&archive, &rows, &origins)?;
    super::animation_clip_references::verify_scoped(&rows)?;
    verify_tag_imports(&rows, &origins)
}

fn fixture(folder: &Path) -> Result<Archive, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(DCB, dataforge()).encrypted())
        .entry(TestEntry::stored(ROOT, ROOT_XML))
        .entry(TestEntry::stored(CHILD, CHILD_XML).encrypted())
        .entry(TestEntry::stored(ATTACHMENT, ATTACHMENT_XML))
        .entry(TestEntry::stored(TAGS, TAGS_XML))
        .entry(TestEntry::stored(IMPORTED_TAGS, IMPORTED_XML).encrypted())
        .entry(TestEntry::stored(EXCLUDED, b"excluded fixture").with_crc32(0))
        .entry(TestEntry::stored(UNRELATED, b"unrelated fixture").with_crc32(0))
        .entry(
            TestEntry::stored("Data/Animations/Scope/ambiguous.adb", b"first candidate")
                .with_crc32(0),
        )
        .entry(
            TestEntry::stored("Data/Animations/Scope/AMBIGUOUS.adb", b"second candidate")
                .with_crc32(0),
        )
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn dataforge() -> Vec<u8> {
    let entity = TestEntity::new(1, ENTITY, "entities/fixture_mannequin_owner.xml")
        .loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("left", PART),
            TestLoadoutEntry::class("right", PART),
        ]))
        .port(TestPort::new("left"))
        .port(TestPort::new("right"))
        .component(selector(ANIMATED, "animDatabase3P", ROOT_SELECTOR))
        .component(selector(ANIMATED, "animDatabase3P", DUPLICATE_SELECTOR));
    let attachment = TestEntity::new(2, PART, "entities/fixture_mannequin_attachment.xml")
        .component(selector(
            CONTROLLER,
            "AnimationDatabase",
            ATTACHMENT_SELECTOR,
        ));
    entity_schema(DataForgeBuilder::new())
        .structure(
            TestStruct::new(ANIMATED)
                .parent("DataForgeComponentParams")
                .field("animDatabase3P", TestType::String),
        )
        .structure(
            TestStruct::new(CONTROLLER)
                .parent("DataForgeComponentParams")
                .field("AnimationDatabase", TestType::String),
        )
        .record(entity.record())
        .record(attachment.record())
        .build()
}

fn selector(component: &str, field: &str, path: &str) -> TestValue {
    TestValue::Struct(TestInstance::new(component).value(field, TestValue::text(path)))
}

fn verify_documents(rows: &[MediaRecord]) {
    let mut documents = typed(rows, "document")
        .filter_map(|(_, row)| match &row.source.identity {
            Identity::Entry { entry, containers } if containers.is_empty() => {
                Some(entry.path.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    documents.sort_unstable();
    assert_eq!(
        documents,
        [ATTACHMENT, CHILD, ROOT],
        "only verified root and reachable child definitions publish once each"
    );
}

struct Binding {
    record: u32,
    component: u64,
    field: &'static str,
    path: &'static str,
    port: Option<&'static str>,
}

const BINDINGS: [Binding; 4] = [
    Binding {
        record: 0,
        component: 2,
        field: "animDatabase3P",
        path: ROOT_SELECTOR,
        port: None,
    },
    Binding {
        record: 0,
        component: 3,
        field: "animDatabase3P",
        path: DUPLICATE_SELECTOR,
        port: None,
    },
    Binding {
        record: 1,
        component: 0,
        field: "AnimationDatabase",
        path: ATTACHMENT_SELECTOR,
        port: Some("left"),
    },
    Binding {
        record: 1,
        component: 0,
        field: "AnimationDatabase",
        path: ATTACHMENT_SELECTOR,
        port: Some("right"),
    },
];

fn verify_bindings(archive: &Archive, rows: &[MediaRecord], origins: &Origins) -> TestResult {
    let dcb_entry = entry_index(archive, DCB)?;
    let bindings = typed(rows, "entity-database-binding").collect::<Vec<_>>();
    assert_eq!(
        bindings.len(),
        BINDINGS.len(),
        "one row per authored occurrence"
    );
    for (position, ((ordinal, row), expected)) in bindings.into_iter().zip(BINDINGS).enumerate() {
        verify_binding(row, position, &expected);
        let proof = origins.proof(ordinal, "/data/payload/authored_path", dcb_entry, true)?;
        assert_eq!(
            proof.pointer("/location/namespace"),
            Some(&json!("dataforge-record")),
            "original DataForge coordinates"
        );
        assert_eq!(
            proof.pointer("/location/indices"),
            Some(&json!([expected.record.to_string()])),
            "original Record position"
        );
        assert_eq!(
            proof.pointer("/location/field"),
            Some(&json!(format!(
                "Components[{}].{}",
                expected.component, expected.field
            ))),
            "original physical component field"
        );
    }
    Ok(())
}

fn verify_binding(row: &MediaRecord, position: usize, expected: &Binding) {
    assert_eq!(
        row.subresource.namespace, "animation-definition-entity-database-binding",
        "selection has its own typed record identity"
    );
    assert_eq!(row.subresource.indices, [position as u64], "source order");
    assert_eq!(
        row.data.pointer("/payload/owner_record_index"),
        Some(&json!(expected.record)),
        "physical owner"
    );
    assert_eq!(
        row.data.pointer("/payload/component_index"),
        Some(&json!(expected.component)),
        "component occurrence"
    );
    assert_eq!(
        datum(row, "/payload/field"),
        expected.field,
        "authored field"
    );
    assert_eq!(
        datum(row, "/payload/authored_path"),
        expected.path,
        "original spelling"
    );
    assert_eq!(
        row.data
            .pointer("/payload/port_path")
            .and_then(Value::as_str),
        expected.port,
        "physical placement"
    );
    assert_eq!(
        datum(row, "/payload/reference/resolution/status"),
        "resolved",
        "verified selected target"
    );
}

fn verify_graph(archive: &Archive, rows: &[MediaRecord], origins: &Origins) -> TestResult {
    let root = entry_index(archive, ROOT)?;
    let child = entry_index(archive, CHILD)?;
    let links = typed(rows, "link")
        .filter(|(_, row)| datum(row, "/payload/kind") == "sub-database")
        .collect::<Vec<_>>();
    let expected = [
        (r"ANIMATIONS\Scope\Child.ADB", "resolved", root, false),
        ("Data/Animations/Scope/child.adb", "resolved", root, false),
        ("Animations/Scope/missing.adb", "missing", root, false),
        ("Animations/Scope/excluded.adb", "not-selected", root, false),
        ("Animations/Scope/ambiguous.adb", "ambiguous", root, false),
        ("Animations/Scope/root.adb", "resolved", child, true),
    ];
    assert_eq!(
        links.len(),
        expected.len(),
        "all authored edges remain addressable"
    );
    for ((ordinal, row), (path, state, source, encrypted)) in links.into_iter().zip(expected) {
        assert_eq!(
            datum(row, "/payload/authored_path"),
            path,
            "source spelling"
        );
        assert_eq!(
            datum(row, "/payload/reference/resolution/status"),
            state,
            "exact frontier state"
        );
        let proof = origins.proof(ordinal, "/data/payload/authored_path", source, encrypted)?;
        assert_eq!(
            proof.pointer("/location/namespace"),
            Some(&json!("animation-definition-xml")),
            "original declaration span"
        );
    }
    verify_cycle(rows, root, child)
}

fn verify_cycle(rows: &[MediaRecord], root: usize, child: usize) -> TestResult {
    let edges: Vec<_> = typed(rows, "sub-database-edge")
        .map(|(_, row)| {
            Ok((
                datum(row, "/payload/source_entry_index").parse::<usize>()?,
                datum(row, "/payload/target_entry_index").parse::<usize>()?,
            ))
        })
        .collect::<Result<_, Box<dyn Error>>>()?;
    assert_eq!(
        edges,
        [(root, child), (root, child), (child, root)],
        "parallel declarations and back edge target the original physical Entries"
    );
    let cycles = typed(rows, "sub-database-cycle").collect::<Vec<_>>();
    let [(_, cycle)] = cycles.as_slice() else {
        return Err("one closed root-child cycle".into());
    };
    assert_eq!(
        datum(cycle, "/payload/member_count"),
        "2",
        "two distinct documents"
    );
    assert_eq!(
        datum(cycle, "/payload/edge_count"),
        "3",
        "parallel graph edges count separately"
    );
    Ok(())
}

fn verify_tag_imports(rows: &[MediaRecord], origins: &Origins) -> TestResult {
    for (spelling, name, path) in [("One", "ONE", IMPORTED_TAGS), ("Two", "Two", TAGS)] {
        let (ordinal, token) = typed(rows, "sub-database-token")
            .find(|(_, row)| datum(row, "/payload/spelling") == spelling)
            .ok_or("authored branch tag")?;
        assert_eq!(
            datum(token, "/payload/resolution"),
            "resolved",
            "the scoped branch resolves through the original TagDef"
        );
        assert!(
            typed(rows, "sub-database-candidate").any(|(_, row)| {
                datum(row, "/payload/name") == name
                    && datum(row, "/payload/target/entry/path") == path
            }),
            "the candidate belongs to the exact direct or imported Entry"
        );
        origins.marker(ordinal, "/data/payload/spelling", false)?;
        origins.marker(ordinal, "/data/payload/resolution", true)?;
    }
    Ok(())
}

fn typed<'a>(
    rows: &'a [MediaRecord],
    kind: &'a str,
) -> impl Iterator<Item = (usize, &'a MediaRecord)> {
    rows.iter()
        .enumerate()
        .filter(move |(_, row)| datum(row, "/record_type") == kind)
}

fn datum<'a>(row: &'a MediaRecord, pointer: &str) -> &'a str {
    row.data
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn entry_index(archive: &Archive, path: &str) -> Result<usize, Box<dyn Error>> {
    archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .map(Entry::index)
        .ok_or_else(|| format!("missing original Entry {path}").into())
}
