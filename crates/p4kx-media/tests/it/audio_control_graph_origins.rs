// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use crate::support;
use crate::support::audio_provider::{Origins, TestResult};

const QUERY: &str = r#"<ATLConfig><AudioPreloads><ATLPreloadRequest atl_name="Request"><RTPCDependees><ATLRTPCDependee atl_name = 'SharedRtpc'/><ATLRTPCDependee atl_name='UnresolvedFixture'/></RTPCDependees></ATLPreloadRequest></AudioPreloads></ATLConfig>"#;
const PLAIN_TARGET: &str =
    r#"<ATLConfig><AudioRTPCs><ATLRtpc atl_name="SharedRtpc"/></AudioRTPCs></ATLConfig>"#;
const ENCRYPTED_NONMATCH: &str =
    r#"<ATLConfig><AudioRTPCs><ATLRtpc atl_name="DifferentRtpc"/></AudioRTPCs></ATLConfig>"#;
const ENCRYPTED_MATCH: &str = r#"<ATLConfig><AudioRTPCs><ATLRtpc unused="keep" atl_name= 'SharedRtpc'/></AudioRTPCs></ATLConfig>"#;

#[test]
fn atl_observed_candidates_cite_only_the_query_and_actual_name_matches() -> TestResult {
    check_observed_matches(
        ENCRYPTED_NONMATCH,
        &[
            (0, "atl_name = 'SharedRtpc'"),
            (1, r#"atl_name="SharedRtpc""#),
        ],
        false,
    )?;
    check_observed_matches(
        ENCRYPTED_MATCH,
        &[
            (0, "atl_name = 'SharedRtpc'"),
            (1, r#"atl_name="SharedRtpc""#),
            (2, "atl_name= 'SharedRtpc'"),
        ],
        true,
    )
}

fn check_observed_matches(
    encrypted_document: &str,
    expected: &[(usize, &str)],
    encrypted: bool,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive_path = scratch.path().join("Data.p4k");
    let documents = [QUERY, PLAIN_TARGET, encrypted_document];
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/Query.xml",
            QUERY.as_bytes(),
        ))
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/Plain.xml",
            PLAIN_TARGET.as_bytes(),
        ))
        .entry(
            TestEntry::stored(
                "Data/Libs/GameAudio/Encrypted.xml",
                encrypted_document.as_bytes(),
            )
            .encrypted(),
        )
        .write_to(&archive_path)?;
    let archive = Archive::open_with(archive_path, OpenOptions::new().key(Some(test_key())))?;
    let output = scratch.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AudioControl],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "audio-control.jsonl")?;
    let (ordinal, summary) = rows
        .iter()
        .enumerate()
        .find(|(_, row)| row.subresource.namespace == "audio-control-reference-resolution")
        .ok_or("summary of the authored lookup")?;
    assert_eq!(
        summary.data.pointer("/payload/candidates_complete"),
        Some(&json!(true)),
        "the encrypted document was admitted and inspected"
    );
    assert_eq!(
        summary.data.pointer("/payload/observed_candidates"),
        Some(&json!((expected.len() - 1).to_string())),
        "each matching physical declaration contributes one candidate"
    );
    let origins = Origins::read(&output.join(&index.artifact_base))?;
    let receipt = origins.field_in(
        "records",
        Some("audio-control"),
        ordinal,
        "/data/payload/observed_candidates",
        encrypted,
    )?;
    check_uses(&origins, receipt, &documents, expected)?;
    check_shared_directory(&output.join(&index.artifact_base), &index, &rows, &origins)
}

fn check_shared_directory(
    base: &std::path::Path,
    index: &p4kx_media::Index,
    rows: &[p4kx_media::MediaRecord],
    origins: &Origins,
) -> TestResult {
    use std::collections::BTreeSet;

    let artifact = &index.field_origins.as_ref().ok_or("field receipt")?.sets;
    let path = base.join(&artifact.path);
    assert_eq!(
        std::fs::metadata(&path)?.len(),
        artifact.size,
        "the index accounts for the entire set artifact"
    );
    let stream: Vec<Value> = support::audio_provider::read_rows(path)?;
    let header = stream.first().ok_or("one framed directory set")?;
    let id = header.get("id").ok_or("shared directory ID")?;
    assert_eq!(
        header["type"], "set",
        "the directory proof starts with a set header"
    );
    assert_eq!(
        stream.last().ok_or("use set EOF")?["sets"],
        "1",
        "all query fields share one directory proof"
    );
    let mut entries = BTreeSet::new();
    for member in stream.iter().filter(|row| row["type"] == "member") {
        let use_ref = member.get("sourceUseRef").ok_or("physical member ref")?;
        let used = origins.physical_use(use_ref)?;
        if used.observation.pointer("/location/namespace") == Some(&json!("atl-definition-file")) {
            let ordinal = used
                .source
                .get("entryIndex")
                .and_then(Value::as_u64)
                .ok_or("physical ATL definition")?;
            entries.insert(ordinal);
        }
    }
    assert_eq!(
        entries,
        BTreeSet::from([0, 1, 2]),
        "a shared proof retains every scanned document"
    );

    check_query_fields(origins, rows, id)
}

fn check_query_fields(
    origins: &Origins,
    rows: &[p4kx_media::MediaRecord],
    id: &Value,
) -> TestResult {
    let mut query_uses = std::collections::BTreeSet::new();
    for (ordinal, row) in rows.iter().enumerate() {
        if row.subresource.namespace != "audio-control-reference-resolution" {
            continue;
        }
        let receipt = origins.field_in(
            "records",
            Some("audio-control"),
            ordinal,
            "/data/payload/resolution",
            true,
        )?;
        assert_eq!(
            receipt.get("sourceUseSetRef"),
            Some(id),
            "the same directory is reused"
        );
        let [local] = receipt["sourceUsesRefs"]
            .as_array()
            .ok_or("local query source")?
            .as_slice()
        else {
            return Err("the original query remains a separate local source".into());
        };
        query_uses.insert(local.as_str().ok_or("original query use ref")?);
    }
    assert_eq!(
        query_uses.len(),
        2,
        "the two queries retain distinct local uses"
    );
    Ok(())
}

fn check_uses(
    origins: &Origins,
    receipt: &Value,
    documents: &[&str],
    expected: &[(usize, &str)],
) -> TestResult {
    let references = receipt
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("query and actual target references")?;
    assert_eq!(
        references.len(),
        expected.len(),
        "no other source contributes to the positive count"
    );
    for &(entry_index, declaration) in expected {
        let document = documents.get(entry_index).ok_or("known fixture entry")?;
        let start = document
            .find(declaration)
            .ok_or("original fixture declaration")?;
        let location = json!({
            "type": "subresource",
            "namespace": "atl-xml",
            "indices": [start.to_string(), (start + declaration.len()).to_string()],
            "field": "declaration",
        });
        let mut found = false;
        for reference in references {
            let used = origins.physical_use(reference)?;
            found |= used.source.get("entryIndex") == Some(&json!(entry_index))
                && used.observation.get("role") == Some(&json!("selector"))
                && used.observation.get("location") == Some(&location);
        }
        assert!(
            found,
            "count must cite {declaration} in physical entry {entry_index}"
        );
    }
    Ok(())
}
