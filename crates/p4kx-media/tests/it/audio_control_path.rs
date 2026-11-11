// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Filter};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord, Source};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::json;

use crate::support;
use crate::support::audio_provider::{TestResult, read_rows};

const PATH: &str = "Data/Sounds/wwise/target.bnk";
const XML: &str = r#"
<ATLConfig><AudioPreloads><ATLPreloadRequest atl_name="Source">
  <WwiseFile wwise_name="Data/Sounds/wwise/target.bnk"/>
  <WwiseFile wwise_name="../target.bnk"/>
  <WwiseFile wwise_name="target.bnk"/>
</ATLPreloadRequest></AudioPreloads></ATLConfig>
"#;

#[test]
fn atl_file_paths_keep_each_exact_candidate_and_refuse_unsafe_or_unproved_spellings() -> TestResult
{
    let scratch = tempfile::tempdir()?;
    let archive_path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/Probe.xml",
            XML.as_bytes(),
        ))
        .entry(TestEntry::stored(PATH, b"locked physical bank").encrypted())
        .entry(TestEntry::stored(PATH, b"second physical bank"))
        .write_to(&archive_path)?;
    let archive = Archive::open(archive_path)?;
    let output = scratch.path().join("out");
    let mut options = ExportOptions {
        kinds: vec![MediaKind::AudioControl],
        include: Some(Filter::parse("*.xml")?),
        ..ExportOptions::default()
    };
    options.limits.reference_candidates = 1;
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let sources: Vec<Source> = read_rows(output.join(&index.artifact_base).join("sources.jsonl"))?;
    assert_eq!(
        sources.len(),
        1,
        "metadata-only path lookup retains neither target payload"
    );
    let rows = support::records(&output, &index, "audio-control.jsonl")?;
    check_paths(&rows)
}

fn check_paths(rows: &[MediaRecord]) -> TestResult {
    let edge = |path: &str| {
        rows.iter().find(|row| {
            row.subresource.namespace == "audio-control-reference"
                && row.data.pointer("/payload/target/path") == Some(&json!(path))
        })
    };
    let exact = edge(PATH).ok_or("authored exact file edge")?;
    assert_eq!(
        exact.data.pointer("/payload/link/resolution/status"),
        Some(&json!("ambiguous")),
        "all exact physical path matches are admitted"
    );
    let candidates: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "audio-control-path-candidate")
        .collect();
    assert_eq!(
        candidates.len(),
        2,
        "streamed path candidates retain both matching physical targets independently of the inline cap"
    );
    for (row, (entry, key)) in candidates.into_iter().zip([(1, true), (2, false)]) {
        assert_eq!(
            row.data.pointer("/payload/target_entry_index"),
            Some(&json!(entry.to_string())),
            "exact physical target order"
        );
        assert_eq!(
            row.data.pointer("/payload/key_required"),
            Some(&json!(key)),
            "directory-only locked access state"
        );
        assert_eq!(
            row.data.pointer("/payload/state/reason"),
            Some(&json!("filter")),
            "target selection remains independent from key requirement"
        );
    }
    for (path, reason) in [
        ("../target.bnk", "unsafe-path"),
        ("target.bnk", "unproved-target-path"),
    ] {
        let row = edge(path).ok_or("authored unresolvable file edge")?;
        assert_eq!(
            row.data.pointer("/payload/link/reason"),
            Some(&json!(reason)),
            "preserve the exact authored target and refusal"
        );
    }
    Ok(())
}
