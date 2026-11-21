// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

use p4kx_assembly::BodyVariant;
use p4kx_blender::{
    PlayerDocument, PlayerDocumentRejection, PlayerDocumentSink, PlayerSource, PlayerSourceCapture,
    RejectedPlayerDocument,
};
use p4kx_extraction::{Archive, CancelToken};
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::TestResult;

const MALFORMED: &str = "Data/Animations/test/malformed.adb";
const WRONG_ROOT: &str = "Data/Animations/test/invalid.adb";
const LOADOUT_MALFORMED: &str = "Data/Scripts/Loadouts/Player/DefaultLoadouts/malformed.xml";
const LOADOUT_ROOT: &str = "Data/Scripts/Loadouts/Player/DefaultLoadouts/root.xml";
const LOADOUT_BAD_CRC: &str = "Data/Scripts/Loadouts/Player/DefaultLoadouts/bad-crc.xml";

#[derive(Default)]
struct Documents {
    projected: Vec<(usize, usize)>,
    rejected: Vec<Rejected>,
}

struct Rejected {
    source: Value,
    entry: usize,
    origin: Value,
    rejection: PlayerDocumentRejection,
    bytes: Vec<u8>,
}

impl PlayerDocumentSink for Documents {
    fn document(&mut self, document: PlayerDocument<'_>) -> io::Result<()> {
        self.projected
            .push((document.original.entry().ordinal() as usize, document.index));
        Ok(())
    }

    fn rejected_document(&mut self, document: RejectedPlayerDocument<'_>) -> io::Result<()> {
        self.rejected.push(Rejected {
            source: serde_json::to_value(document.source)?,
            entry: document.original.entry().ordinal() as usize,
            origin: serde_json::to_value(document.origin)?,
            rejection: document.rejection,
            bytes: document.bytes.to_vec(),
        });
        Ok(())
    }
}

fn fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let root = super::subadb_graph_assertions::ROOT_LINKS
        .strip_suffix("</SubADBs>")
        .ok_or("root SubADB wrapper")?;
    let links = format!(
        r#"{root}
        <SubADB File="Animations/test/malformed.adb" Tags="one"/>
        <SubADB File="DATA/ANIMATIONS/TEST/MALFORMED.ADB" Tags="two"/>
        <SubADB File="DATA/ANIMATIONS/TEST/INVALID.ADB" Tags="again"/>
        </SubADBs>"#
    );
    super::fixture_with(
        &links,
        [
            TestEntry::stored(MALFORMED, br"<AnimDB>").encrypted(),
            TestEntry::stored(LOADOUT_MALFORMED, br"<Loadout>").encrypted(),
            TestEntry::stored(LOADOUT_ROOT, br"<DifferentRoot/>"),
            TestEntry::stored(LOADOUT_BAD_CRC, br"<Loadout/>").with_crc32c(0),
        ],
    )
}

#[test]
fn verified_rejections_share_physical_originals_without_admitting_failed_reads() -> TestResult {
    let (_folder, archive) = fixture()?;
    let cancel = CancelToken::new();
    let body = BodyVariant::Male;
    let ordinary = serde_json::to_value(PlayerSource::read(&archive, body, &cancel)?)?;
    let mut sink = Documents::default();
    let readers = PlayerSourceCapture::read(&archive, &[body], &cancel, &mut sink)?;
    let captured = serde_json::to_value(readers)?;
    for key in ["loadouts", "motion"] {
        assert_eq!(
            captured.get(key),
            ordinary.get(key),
            "unchanged rows and refs"
        );
    }
    super::shared_capture::same_legacy_inventory(&ordinary, &captured)?;
    verify_rejections(&sink, &archive, &captured)?;
    verify_graph(&captured, &sink, &archive)?;
    verify_loadouts(&captured, &sink, &archive)?;
    Ok(())
}

fn verify_rejections(sink: &Documents, archive: &Archive, captured: &Value) -> TestResult {
    assert_eq!(sink.rejected.len(), 4, "one receipt per rejected source");
    for (path, invalid, encrypted) in [
        (MALFORMED, true, true),
        (WRONG_ROOT, false, false),
        (LOADOUT_MALFORMED, true, true),
        (LOADOUT_ROOT, false, false),
    ] {
        let record = record(sink, archive, path)?;
        let entry = archive
            .entries()
            .get(record.entry)
            .ok_or("physical entry")?;
        let mut exact = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut exact)?;
        assert_eq!(record.bytes, exact, "{path} exact verified original");
        assert_eq!(
            record.rejection,
            if invalid {
                PlayerDocumentRejection::MalformedXml
            } else {
                PlayerDocumentRejection::UnexpectedRoot {
                    format: "textXml",
                    lossy: false,
                }
            },
            "{path} parser disposition",
        );
        assert_eq!(
            record.origin.get("sourceSetComplete"),
            Some(&json!(true)),
            "{path} exact use"
        );
        assert_eq!(
            record.origin.get("containsEncryptedSource"),
            Some(&json!(encrypted)),
            "{path} encryption"
        );
        verify_evidence(record, captured, path, invalid)?;
    }
    Ok(())
}

fn verify_evidence(record: &Rejected, captured: &Value, path: &str, malformed: bool) -> TestResult {
    let use_ref = record
        .origin
        .pointer("/sourceUsesRefs/0")
        .ok_or("rejected evidence")?;
    let used = captured
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .and_then(|rows| rows.iter().find(|row| row.get("id") == Some(use_ref)))
        .ok_or("registered rejected evidence")?;
    assert_eq!(
        used.get("source"),
        Some(&record.source),
        "{path} own source"
    );
    assert_eq!(
        used.get("role"),
        Some(&json!("payload")),
        "{path} read rather than attempted"
    );
    let coordinate = if malformed { "metadata" } else { "subresource" };
    assert_eq!(
        used.pointer("/location/type"),
        Some(&json!(coordinate)),
        "{path} observed payload or root"
    );
    Ok(())
}

fn verify_graph(captured: &Value, sink: &Documents, archive: &Archive) -> TestResult {
    let links = captured
        .pointer("/motion/components/value/0/databaseGraph/branches/0/links")
        .and_then(Value::as_array)
        .ok_or("original graph choices")?;
    let targets = |path| -> TestResult<Vec<&Value>> {
        let rejected = record(sink, archive, path)?;
        Ok(links
            .iter()
            .filter(|link| link.pointer("/target/value/source") == Some(&rejected.source))
            .collect())
    };
    for path in [MALFORMED, WRONG_ROOT] {
        let selected = targets(path)?;
        assert_eq!(selected.len(), 2, "{path} both original choices remain");
        for link in selected {
            assert_eq!(
                link.pointer("/target/value/type"),
                Some(&json!("invalidXml")),
                "{path} explicit rejection"
            );
            assert_eq!(
                link.pointer("/traversal/value"),
                Some(&json!("unresolved")),
                "{path} no graph branch"
            );
        }
    }
    assert!(
        links
            .iter()
            .any(|link| link.pointer("/target/value/type") == Some(&json!("unreadable"))),
        "bad CRC graph choice stays unreadable"
    );
    Ok(())
}

fn verify_loadouts(captured: &Value, sink: &Documents, archive: &Archive) -> TestResult {
    let rows = captured
        .get("loadouts")
        .and_then(Value::as_array)
        .ok_or("loadout rows")?;
    for (path, has_document) in [(LOADOUT_MALFORMED, false), (LOADOUT_ROOT, true)] {
        let original = record(sink, archive, path)?;
        let row = rows
            .iter()
            .find(|row| row.pointer("/value/source") == Some(&original.source))
            .ok_or("loadout rejected source")?;
        assert_eq!(
            row.pointer("/value/type"),
            Some(&json!("invalidXml")),
            "{path} rejection"
        );
        let established = sink
            .projected
            .iter()
            .find_map(|(entry, index)| (*entry == original.entry).then_some(*index));
        assert_eq!(
            established.is_some(),
            has_document,
            "{path} projected index"
        );
        assert_eq!(
            row.pointer("/value/document"),
            Some(&json!(established)),
            "{path} legacy document index"
        );
    }
    assert!(
        rows.iter()
            .any(|row| row.pointer("/value/type") == Some(&json!("unreadable"))),
        "bad CRC loadout stays unreadable"
    );
    Ok(())
}

fn record<'a>(sink: &'a Documents, archive: &Archive, path: &str) -> TestResult<&'a Rejected> {
    sink.rejected
        .iter()
        .find(|row| {
            archive
                .entries()
                .get(row.entry)
                .is_some_and(|entry| entry.path() == path)
        })
        .ok_or_else(|| format!("missing verified rejection {path}").into())
}

struct RefuseRejected;

impl PlayerDocumentSink for RefuseRejected {
    fn document(&mut self, _document: PlayerDocument<'_>) -> io::Result<()> {
        Ok(())
    }

    fn rejected_document(&mut self, _document: RejectedPlayerDocument<'_>) -> io::Result<()> {
        Err(io::Error::other("test rejected-document sink failure"))
    }
}

#[test]
fn rejected_document_sink_failure_is_a_failed_capture() -> TestResult {
    let (_folder, archive) = fixture()?;
    let result = PlayerSourceCapture::read(
        &archive,
        &[BodyVariant::Male],
        &CancelToken::new(),
        &mut RefuseRejected,
    );
    assert!(
        result.is_err_and(|error| error.to_string().contains("retain")),
        "a sink that cannot retain the bytes prevents successful capture",
    );
    Ok(())
}
