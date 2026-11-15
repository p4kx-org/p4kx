// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Entry, Exclusion, Filter, OpenOptions};
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntity,
    TestEntry, TestLoadout, TestModel, entity_schema, test_key,
};
use serde_json::{Value, json};

use super::support::{self, TestResult};

const ENTITY: &str = "FixtureXmlRoot";
const ROOT: &str = "Data/Scripts/Loadouts/ScopeRoot.xml";
const PARTS: &str = "Data/Scripts/Loadouts/ScopeParts.xml";
const PARAMETERS: &str = "Data/Objects/xml-child.chrparams";
const EVENTS: &str = "Data/Animations/Xml/events.animevents";
const STREAM: &str = "animation-definition.jsonl";

#[test]
fn scoped_entity_follows_an_authored_loadout_include_beyond_the_filter() -> TestResult {
    let temp = tempfile::tempdir()?;
    let archive = fixture(temp.path(), IncludedXml::Valid)?;
    let output = temp.path().join("media");
    let index = support::published(p4kx_media::export(&archive, &output, &options()?)?)?;
    let rows = support::records(&output, &index, STREAM)?;
    verify_placements(&archive, &rows)?;
    verify_definitions(&archive, &rows)?;
    verify_port_origins(&archive, &rows, &output.join(&index.artifact_base))
}

#[test]
fn scoped_entity_requires_a_unique_readable_non_excluded_loadout_include() -> TestResult {
    let cases = [
        IncludedXml::Missing,
        IncludedXml::Ambiguous,
        IncludedXml::Corrupt,
        IncludedXml::Excluded,
    ];
    for case in cases {
        let temp = tempfile::tempdir()?;
        let archive = fixture(temp.path(), case)?;
        let output = temp.path().join("media");
        let mut options = options()?;
        if matches!(case, IncludedXml::Excluded) {
            options.exclude = Exclusion::parse([PARTS])?;
        }
        let failure = p4kx_media::export(&archive, &output, &options)
            .err()
            .ok_or_else(|| format!("{case:?} loadout include produced a complete index"))?;
        verify_failure(&failure, case);
        assert!(
            !output.join("media-index.json").exists(),
            "failed scope has no complete generation"
        );
    }
    Ok(())
}

fn options() -> Result<ExportOptions, Box<dyn Error>> {
    Ok(ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        animation_entity: Some(ENTITY.into()),
        include: Some(Filter::parse("Data/unrelated.dat")?),
        ..ExportOptions::default()
    })
}

fn verify_placements(archive: &Archive, rows: &[MediaRecord]) -> TestResult {
    let placements: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "animation-definition-scope-receipt")
        .collect();
    let [root, left, right] = placements.as_slice() else {
        return Err("expected the root and both XML-supplied attachment occurrences".into());
    };
    assert_eq!(
        root.data.pointer("/payload/placement"),
        Some(&json!("entity-root")),
        "root Entity receipt remains distinct"
    );
    assert_eq!(
        root.data.pointer("/payload/outcome/state"),
        Some(&json!("no-rig")),
        "root Entity supplies a Loadout without a rig"
    );
    let parameter_entry = entry_index(archive, PARAMETERS)?.to_string();
    for (ordinal, port, row) in [(1, "sharedLeft", *left), (2, "sharedRight", *right)] {
        assert_eq!(
            row.subresource.indices,
            [ordinal],
            "physical occurrence order"
        );
        assert_eq!(
            row.data.pointer("/payload/placement"),
            Some(&json!("attachment")),
            "existing attachment discriminator is preserved"
        );
        assert_eq!(
            row.data.pointer("/payload/placement_detail"),
            Some(&json!({"kind":"attachment", "port_path":port})),
            "each XML port retains its own authored name",
        );
        assert_eq!(
            row.data.pointer("/payload/owner_record_index"),
            Some(&json!(1)),
            "both ports attach the same physical Record"
        );
        assert_eq!(
            row.data.pointer("/payload/outcome"),
            Some(&json!({"state":"selected", "entry_index":parameter_entry})),
            "both occurrences select the same physical parameter Entry",
        );
    }
    Ok(())
}

fn verify_definitions(archive: &Archive, rows: &[MediaRecord]) -> TestResult {
    let mut definitions: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "animation-definition-document")
        .filter_map(|row| match &row.source.identity {
            Identity::Entry { entry, containers } if containers.is_empty() => {
                Some(entry.entry_index)
            }
            _ => None,
        })
        .collect();
    definitions.sort_unstable();
    let mut expected = [
        entry_index(archive, PARAMETERS)?,
        entry_index(archive, EVENTS)?,
    ];
    expected.sort_unstable();
    assert_eq!(
        definitions, expected,
        "shared parameters and their authored event database each export once",
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.subresource.namespace == "animation-definition-event-binding")
            .count(),
        1,
        "ordinary exact clip linkage still follows the scoped definition",
    );
    Ok(())
}

fn verify_port_origins(archive: &Archive, rows: &[MediaRecord], base: &Path) -> TestResult {
    let sources = json_rows(base, "origin-sources.jsonl")?;
    let uses = json_rows(base, "origin-uses.jsonl")?;
    let fields = json_rows(base, "field-origins.jsonl")?;
    let included_index = entry_index(archive, PARTS)?;
    let included = sources
        .iter()
        .find(|source| source["entryIndex"] == json!(included_index))
        .ok_or("physical encrypted Loadout Entry evidence")?;
    let mut checked = 0;
    for (ordinal, row) in rows.iter().enumerate() {
        let node = match row
            .data
            .pointer("/payload/placement_detail/port_path")
            .and_then(Value::as_str)
        {
            Some("sharedLeft") => "2",
            Some("sharedRight") => "3",
            _ => continue,
        };
        let origin = field_origin(&fields, ordinal, "/data/payload/placement_detail/port_path")?;
        verify_class_origins(&fields, &uses, ordinal, node)?;
        assert_eq!(
            origin["containsEncryptedSource"], true,
            "encrypted port name"
        );
        assert_eq!(
            origin["sourceSetComplete"], true,
            "original port selector retained"
        );
        let refs = origin["sourceUsesRefs"]
            .as_array()
            .ok_or("port selector uses")?;
        assert!(
            uses.iter().any(|used| {
                refs.contains(&used["id"])
                    && used["source"] == included["id"]
                    && used["scope"] == "payload"
                    && used["role"] == "selector"
                    && used.pointer("/location/namespace").and_then(Value::as_str)
                        == Some("loadout-cryxml-attribute")
                    && used.pointer("/location/field").and_then(Value::as_str) == Some("portName")
                    && used.pointer("/location/indices") == Some(&json!([node, "0"]))
            }),
            "port name comes from its own original binary XML node and attribute",
        );
        checked += 1;
    }
    assert_eq!(
        checked, 2,
        "both repeated port occurrences have exact evidence"
    );
    Ok(())
}

fn verify_class_origins(
    fields: &[Value],
    uses: &[Value],
    ordinal: usize,
    node: &str,
) -> TestResult {
    for field in [
        "/data/payload/owner_record_index",
        "/data/payload/outcome/entry_index",
    ] {
        let origin = field_origin(fields, ordinal, field)?;
        assert_eq!(
            origin.get("containsEncryptedSource"),
            Some(&json!(true)),
            "the selected class for {field} comes from encrypted XML"
        );
        let refs = origin
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("class selector uses")?;
        assert!(
            uses.iter()
                .any(|item| item.get("id").is_some_and(|id| refs.contains(id))
                    && item.pointer("/location/namespace")
                        == Some(&json!("loadout-cryxml-attribute"))
                    && item.pointer("/location/field") == Some(&json!("itemName"))
                    && item.pointer("/location/indices/0") == Some(&json!(node))),
            "the {field} choice cites the original class item"
        );
        assert!(
            uses.iter()
                .any(|item| item.get("id").is_some_and(|id| refs.contains(id))
                    && item.pointer("/location/namespace")
                        == Some(&json!("loadout-xml-attribute"))),
            "the {field} choice cites its actual authored Include"
        );
    }
    Ok(())
}

fn field_origin<'a>(
    fields: &'a [Value],
    ordinal: usize,
    field: &str,
) -> Result<&'a Value, Box<dyn Error>> {
    let ordinal = ordinal.to_string();
    fields
        .iter()
        .find(|row| {
            row.pointer("/scope/stream/kind").and_then(Value::as_str)
                == Some("animation-definition")
                && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal.as_str())
                && row["field"] == field
        })
        .ok_or_else(|| "missing placement field origin".into())
}

fn json_rows(base: &Path, stream: &str) -> Result<Vec<Value>, Box<dyn Error>> {
    fs::read_to_string(base.join(stream))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn verify_failure(failure: &dyn Error, case: IncludedXml) {
    let mut current = Some(failure);
    let mut actual = String::new();
    while let Some(error) = current {
        actual.push_str(&error.to_string());
        actual.push('\n');
        current = error.source();
    }
    let normalized = actual.to_ascii_lowercase().replace('\\', "/");
    assert!(
        normalized.contains("scripts/loadouts/scopeparts.xml"),
        "include identity: {actual}"
    );
    let checksum = normalized.contains("checksum") || normalized.contains(" computed,");
    let expected = match case {
        IncludedXml::Corrupt => checksum,
        IncludedXml::Missing => {
            normalized.contains("missing") || normalized.contains("no such file")
        }
        IncludedXml::Excluded => {
            normalized.contains("excluded") || normalized.contains("not selected")
        }
        IncludedXml::Ambiguous => normalized.contains("ambiguous"),
        IncludedXml::Valid => false,
    };
    assert!(expected, "expected distinct {case:?} failure: {actual}");
    if matches!(case, IncludedXml::Ambiguous | IncludedXml::Excluded) {
        assert!(!checksum, "refused candidate was CRC-checked: {actual}");
    }
}

fn entry_index(archive: &Archive, path: &str) -> Result<usize, Box<dyn Error>> {
    archive
        .entries()
        .iter()
        .find(|entry| entry.path().replace('\\', "/") == path)
        .map(Entry::index)
        .ok_or_else(|| format!("missing fixture Entry {path}").into())
}

#[derive(Clone, Copy, Debug)]
enum IncludedXml {
    Valid,
    Missing,
    Ambiguous,
    Corrupt,
    Excluded,
}

fn fixture(root: &Path, included: IncludedXml) -> Result<Archive, Box<dyn Error>> {
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", dataforge()))
        .entry(TestEntry::stored(
            ROOT,
            br#"<Loadout><Loadouts><Loadout loadout="Scripts/Loadouts/sCoPePaRtS.xml"/></Loadouts></Loadout>"#,
        ))
        .entry(TestEntry::stored("Data/Objects/xml-child.chr", TestModel::cube().cgf()))
        .entry(TestEntry::stored(
            PARAMETERS,
            br#"<Params><AnimationList>
              <Animation name="child" path="Animations/Xml/child.caf"/>
              <Animation name="$AnimEventDatabase" path="Animations/Xml/events.animevents"/>
            </AnimationList></Params>"#,
        ))
        .entry(TestEntry::stored(
            EVENTS,
            br#"<anim_event_list><animation name="Animations/Xml/child.caf">
              <event name="cue" time="0.5"/>
            </animation></anim_event_list>"#,
        ))
        .entry(TestEntry::stored("Data/Unrelated/ScopeParts.xml", b"wrong path").with_crc32c(1));
    if !matches!(included, IncludedXml::Missing) {
        let entry = TestEntry::stored(PARTS, parts_xml());
        let entry = if matches!(included, IncludedXml::Corrupt | IncludedXml::Excluded) {
            entry.with_crc32c(1)
        } else {
            entry.encrypted()
        };
        archive = archive.entry(entry);
    }
    if matches!(included, IncludedXml::Ambiguous) {
        archive = archive.entry(
            TestEntry::stored("data/scripts/loadouts/SCOPEPARTS.XML", b"ambiguous").with_crc32c(1),
        );
    }
    let path = root.join("Data.p4k");
    archive.write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn dataforge() -> Vec<u8> {
    let root = TestEntity::new(1, ENTITY, "entities/xml_root.xml")
        .loadout(TestLoadout::Xml(r"Scripts\Loadouts\scoperoot.xml".into()));
    let child = TestEntity::new(2, "FixtureXmlChild", "entities/xml_child.xml")
        .geometry("Objects/xml-child.chr", None);
    entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(child.record())
        .build()
}

fn parts_xml() -> Vec<u8> {
    let item = |port| {
        TestElement::new("Item")
            .attribute("portName", port)
            .attribute("itemName", "FixtureXmlChild")
    };
    TestElement::new("Loadout")
        .child(
            TestElement::new("Items")
                .child(item("sharedLeft"))
                .child(item("sharedRight")),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}
