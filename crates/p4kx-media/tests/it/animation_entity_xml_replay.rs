// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Entry, ExtractionError, Filter, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestAnimation, TestElement,
    TestEntity, TestEntry, TestInstance, TestLoadout, TestModel, TestSkeleton, TestTrack,
    TestValue, entity_schema, test_key,
};
use serde_json::{Value, json};

use super::animation_definition_origins::Origins;
use super::support::{TestResult, published, records};

const DCB: &str = "Data/Game2.dcb";
const ROOT: &str = "Data/GameFixture/Root.xml";
const CHILDREN: &str = "Data/GameFixture/Children.xml";
const BAD: &str = "Data/GameFixture/Bad.xml";
const OUTSIDE: &str = "Data/Scripts/Omitted.xml";
const CAF: &str = "Data/GameFixture/clip.caf";
const LOADOUT: &str = "GameFixture/Root.xml";
const STREAM: &str = "animation.jsonl";

#[derive(Clone, Copy)]
enum Branch {
    None,
    Malformed,
    Missing,
    Ambiguous,
    FilteredChecksum,
    SelectedChecksum,
}

#[test]
fn retained_xml_preserves_both_attachment_origins_and_independent_clip_proof() -> TestResult {
    let sandbox = tempfile::tempdir()?;
    let archive = fixture(sandbox.path(), Branch::None)?;
    let output = sandbox.path().join("media");
    let index = published(p4kx_media::export(&archive, &output, &options()?)?)?;
    let rows = records(&output, &index, STREAM)?;
    let generation = output.join(&index.artifact_base);
    let origins = Origins::read_stream(&generation, "animation")?;
    let attached = verify_attachments(&rows)?;
    verify_owner_origin(&archive, &generation, &attached)?;
    let xml_entry = entry_index(&archive, CHILDREN)?;
    for (position, node) in attached.into_iter().zip(["2", "3"]) {
        let origin = origins.proof(
            position,
            "/data/payload/placement/port_path",
            xml_entry,
            true,
        )?;
        assert_eq!(
            origin["location"],
            json!({"type":"subresource","namespace":"loadout-cryxml-attribute","indices":[node,"0"],"field":"portName"}),
            "each emitted port name uses its own physical authored XML attribute"
        );
    }
    let clip = rows
        .iter()
        .position(|row| row.subresource.namespace == "animation-entity-clip")
        .ok_or("the XML attachment led to no physical clip")?;
    let sources = origins.entry_indices(clip, "/data/payload/frames_per_second", false)?;
    assert_eq!(
        sources.into_iter().collect::<Vec<_>>(),
        [entry_index(&archive, CAF)?],
        "the clip's numerical timing is independent of the encrypted Loadout"
    );
    Ok(())
}

#[test]
fn refused_or_unavailable_xml_retains_the_other_placements_and_precise_attempt() -> TestResult {
    for (branch, code) in [
        (Branch::Malformed, "animation.entity.loadout.malformed"),
        (Branch::Missing, "animation.entity.loadout.missing"),
        (Branch::Ambiguous, "animation.entity.loadout.ambiguous"),
        (
            Branch::FilteredChecksum,
            "animation.entity.loadout.not-selected",
        ),
    ] {
        let sandbox = tempfile::tempdir()?;
        let archive = fixture(sandbox.path(), branch)?;
        let output = sandbox.path().join("media");
        let ExportOutcome::Incomplete { diagnostic, .. } =
            p4kx_media::export(&archive, &output, &options()?)?
        else {
            return Err(format!("XML issue {code} did not keep a diagnostic generation").into());
        };
        let generation = diagnostic
            .parent()
            .ok_or("the diagnostic generation is missing")?;
        let rows = diagnostic_rows(generation)?;
        verify_attachments(&rows)?;
        let (record, summary) = rows
            .iter()
            .enumerate()
            .find(|(_, row)| row.subresource.namespace == "animation-entity-catalogue")
            .ok_or("the Entity catalogue summary is missing")?;
        let issue = summary
            .issues
            .iter()
            .position(|issue| issue.code == code)
            .ok_or_else(|| format!("the actual attempted file must report {code}"))?;
        verify_attempt_marker(generation, record, issue, branch)?;
        assert!(
            !output.join("media-index.json").exists(),
            "partial scope stays diagnostic"
        );
        if matches!(branch, Branch::FilteredChecksum) {
            let index = entry_index(&archive, OUTSIDE)?;
            let sources = json_lines(generation, "origin-sources.jsonl")?;
            assert!(
                !sources
                    .iter()
                    .any(|source| source["entryIndex"] == json!(index)),
                "an unselected CRC-poisoned XML supplies no physical payload evidence"
            );
        }
    }
    Ok(())
}

fn verify_attempt_marker(
    generation: &Path,
    record: usize,
    issue: usize,
    branch: Branch,
) -> TestResult {
    let scope = json!(record.to_string());
    let address = json!(format!("/issues/{issue}"));
    let origins = json_lines(generation, "field-origins.jsonl")?;
    let proof = origins
        .iter()
        .find(|origin| {
            origin.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("animation")
                && origin.pointer("/scope/ordinal") == Some(&scope)
                && origin["field"] == address
        })
        .ok_or("the attempted XML input has no per-issue provenance")?;
    assert_eq!(
        proof["sourceSetComplete"],
        matches!(branch, Branch::Malformed),
        "only the verified parse failure has observed result evidence"
    );
    let uses = proof["sourceUsesRefs"]
        .as_array()
        .ok_or("authored attempt selector references")?;
    assert!(
        !uses.is_empty(),
        "the known original path selector is still retained"
    );
    Ok(())
}

#[test]
fn included_crc_failure_on_xml_remains_an_original_terminal_input_failure() -> TestResult {
    let sandbox = tempfile::tempdir()?;
    let archive = fixture(sandbox.path(), Branch::SelectedChecksum)?;
    let output = sandbox.path().join("media");
    let error = p4kx_media::export(&archive, &output, &options()?)
        .err()
        .ok_or("a selected original CRC failure must terminate export")?;
    let mut original: Option<&(dyn Error + 'static)> = Some(&error);
    let mut checksum = false;
    while let Some(cause) = original {
        checksum |= matches!(
            cause.downcast_ref::<ExtractionError>(),
            Some(ExtractionError::ChecksumMismatch { .. })
        );
        original = cause.source();
    }
    assert!(
        checksum,
        "the original archive verification cause stays typed"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "no complete export after CRC failure"
    );
    Ok(())
}

fn options() -> Result<ExportOptions, Box<dyn Error>> {
    Ok(ExportOptions {
        kinds: vec![MediaKind::Animation],
        animation_entity: Some("XmlRigRoot".into()),
        include: Some(Filter::parse("Data/Game*")?),
        diagnostic: true,
        ..ExportOptions::default()
    })
}

fn verify_attachments(rows: &[MediaRecord]) -> Result<[usize; 2], Box<dyn Error>> {
    let matching: Vec<_> = rows
        .iter()
        .enumerate()
        .filter_map(|(ordinal, row)| {
            if row.subresource.namespace != "animation-entity-rig" {
                return None;
            }
            let port = row.data.pointer("/payload/placement/port_path")?.as_str()?;
            Some((ordinal, port))
        })
        .collect();
    let [(left, "fixtureLeft"), (right, "fixtureRight")] = matching.as_slice() else {
        return Err(
            "both independently placed XML child rigs must survive in authored order".into(),
        );
    };
    Ok([*left, *right])
}

fn verify_owner_origin(archive: &Archive, generation: &Path, attached: &[usize; 2]) -> TestResult {
    let fields = json_lines(generation, "field-origins.jsonl")?;
    let uses = json_lines(generation, "origin-uses.jsonl")?;
    let sources = json_lines(generation, "origin-sources.jsonl")?;
    for (ordinal, node) in attached.iter().zip(["2", "3"]) {
        let receipt = fields
            .iter()
            .find(|field| {
                field.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("animation")
                    && field.pointer("/scope/ordinal") == Some(&json!(ordinal.to_string()))
                    && field["field"] == "/data/payload/owner/guid"
            })
            .ok_or("the selected rig has no authoring proof")?;
        assert_eq!(
            receipt["containsEncryptedSource"], true,
            "the encrypted actual class contributes"
        );
        assert_eq!(
            receipt["sourceSetComplete"], false,
            "unknown class provenance stays explicit"
        );
        let ids = receipt["sourceUsesRefs"]
            .as_array()
            .ok_or("the original owner uses are missing")?;
        for (path, namespace, field) in [
            (DCB, "dataforgeValue", "$value"),
            (ROOT, "loadout-xml-attribute", "loadout"),
            (CHILDREN, "loadout-cryxml-attribute", "itemName"),
        ] {
            let index = entry_index(archive, path)?;
            let identity = sources
                .iter()
                .find(|row| row["entryIndex"] == json!(index))
                .and_then(|row| row.get("id"))
                .ok_or("original archive input for rig")?;
            assert!(
                uses.iter().any(|row| {
                    ids.contains(&row["id"])
                        && row.get("source") == Some(identity)
                        && row.pointer("/location/namespace").and_then(Value::as_str)
                            == Some(namespace)
                        && row.pointer("/location/field").and_then(Value::as_str) == Some(field)
                        && (path != CHILDREN
                            || row.pointer("/location/indices/0") == Some(&json!(node)))
                }),
                "owner selection retains each actual DataForge and XML use"
            );
        }
    }
    Ok(())
}

fn fixture(folder: &Path, branch: Branch) -> Result<Archive, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::stored(DCB, dataforge()))
        .entry(TestEntry::stored(ROOT, root_xml(branch)?))
        .entry(TestEntry::deflated(CHILDREN, parts_xml()).encrypted())
        .entry(TestEntry::stored("Data/GameFixture/child.chr", TestModel::cube().with_skeleton(&TestSkeleton::two_bones()).cgf()))
        .entry(TestEntry::stored("Data/GameFixture/child.chrparams",
              format!(r#"<Params><AnimationList><Animation name="clip" path="{CAF}"/></AnimationList></Params>"#)))
        .entry(TestEntry::stored(CAF, TestAnimation::new("clip", 24)
            .track(TestTrack::new(41).positions(&[(0, [0.0, 0.0, 0.0]), (12, [1.0, 0.0, 0.0])])).caf()));
    archive = match branch {
        Branch::None | Branch::Missing => archive,
        Branch::Malformed => archive.entry(TestEntry::stored(BAD, b"<broken").encrypted()),
        Branch::Ambiguous => archive
            .entry(TestEntry::stored(BAD, b"<Loadout/>"))
            .entry(TestEntry::stored(BAD, b"<Loadout/>").encrypted()),
        Branch::FilteredChecksum => {
            archive.entry(TestEntry::stored(OUTSIDE, b"<Loadout/>").with_crc32c(1))
        }
        Branch::SelectedChecksum => {
            archive.entry(TestEntry::stored(BAD, b"<Loadout/>").with_crc32c(1))
        }
    };
    archive.write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn dataforge() -> Vec<u8> {
    let decoy = TestInstance::new("SItemPortLoadoutXMLParams")
        .value("loadoutPath", TestValue::text(LOADOUT));
    entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "XmlRigRoot", "fixture/root.xml")
                .loadout(TestLoadout::Xml(LOADOUT.into()))
                .component(TestValue::Struct(
                    TestInstance::new("SEntityComponentDefaultLoadoutParams")
                        .value("loadout", TestValue::Struct(decoy)),
                ))
                .record(),
        )
        .record(
            TestEntity::new(2, "XmlRigChild", "fixture/child.xml")
                .geometry("GameFixture/child.chr", None)
                .record(),
        )
        .build()
}

fn root_xml(branch: Branch) -> Result<String, std::fmt::Error> {
    let mut document = String::from("<Loadout><Loadouts>");
    let bad = match branch {
        Branch::None => None,
        Branch::FilteredChecksum => Some("Scripts/Omitted.xml"),
        _ => Some("GameFixture/Bad.xml"),
    };
    if let Some(path) = bad {
        write!(document, r#"<Loadout loadout="{path}"/>"#)?;
    }
    document.push_str(r#"<Loadout loadout="GameFixture/Children.xml"/></Loadouts></Loadout>"#);
    Ok(document)
}

fn parts_xml() -> Vec<u8> {
    let part = |name| {
        TestElement::new("Item")
            .attribute("portName", name)
            .attribute("itemName", "XmlRigChild")
    };
    TestElement::new("Loadout")
        .child(
            TestElement::new("Items")
                .child(part("fixtureLeft"))
                .child(part("fixtureRight")),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn diagnostic_rows(generation: &Path) -> Result<Vec<MediaRecord>, Box<dyn Error>> {
    fs::read_to_string(generation.join(STREAM))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn json_lines(generation: &Path, stream: &str) -> Result<Vec<Value>, Box<dyn Error>> {
    fs::read_to_string(generation.join(stream))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn entry_index(archive: &Archive, path: &str) -> Result<usize, Box<dyn Error>> {
    archive
        .entries()
        .iter()
        .find(|entry| entry.path().replace('\\', "/") == path)
        .map(Entry::index)
        .ok_or_else(|| "the original fixture Entry is missing".into())
}
