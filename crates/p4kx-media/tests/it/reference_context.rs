// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, ConflictPolicy, Filter};
use p4kx_media::{
    CandidateInventory, CandidateState, Capability, ContainerIdentity, ExportOptions, Identity,
    MediaError, MediaKind, MediaWriter, NotSelectedReason, OriginalIdentifier, Projection,
    Provider, ProviderRegistry, ReferenceCandidate, ReferenceTarget, ReferenceUnavailable,
    SourceContext, Status, Subresource,
};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

use super::providers::registry;
use super::support::{self, TestResult};

#[test]
fn both_providers_keep_nested_and_duplicate_physical_reference_targets() -> TestResult {
    let folder = tempfile::tempdir()?;
    let nested = TestContainer::new()
        .entry(TestEntry::stored("same.bin", b"first"))
        .entry(TestEntry::stored("same.bin", b"second"))
        .build();
    let outer = TestContainer::new()
        .entry(TestEntry::stored("nested.pak", nested))
        .build();
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("world.socpak", outer))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let output = folder.path().join("media");
    let options = ExportOptions {
        include: Some(Filter::parse("same.bin")?),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &options,
        &registry()?,
    )?)?;
    for stream in ["material.jsonl", "animation.jsonl"] {
        let rows = support::records(&output, &index, stream)?;
        assert_nested_targets(rows)?;
    }
    Ok(())
}

fn assert_nested_targets(rows: Vec<p4kx_media::MediaRecord>) -> TestResult {
    let mut physical = std::collections::BTreeSet::new();
    for row in rows {
        let Identity::Entry { containers, entry } = &row.source.identity else {
            return Err("physical reference source".into());
        };
        assert_eq!(
            containers
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            ["world.socpak", "nested.pak"],
            "reference source keeps outer-to-inner container order"
        );
        physical.insert(entry.entry_index);
        let target = row
            .data
            .pointer("/references/0/resolution/target/source")
            .ok_or("nested reference target")?;
        assert_eq!(
            *target,
            serde_json::to_value(&row.source.identity)?,
            "reference targets the exact physical source"
        );
        let candidates = row
            .data
            .pointer("/references/2/resolution/candidates")
            .and_then(serde_json::Value::as_array)
            .ok_or("duplicate references")?;
        assert_eq!(
            candidates.len(),
            2,
            "both physical subresources remain ambiguity candidates"
        );
        assert_eq!(
            candidates
                .first()
                .and_then(|candidate| candidate.pointer("/target/source")),
            Some(target),
            "the first candidate retains the same complete source identity"
        );
    }
    assert_eq!(
        physical.into_iter().collect::<Vec<_>>(),
        [0, 1],
        "duplicate display paths keep distinct physical indices"
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum ProbeMode {
    Filtered,
    Incomplete,
    ForeignRoot,
}

struct ReferenceProbe {
    target: ContainerIdentity,
    mode: ProbeMode,
}

impl Provider for ReferenceProbe {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Material,
            "synthetic.reference-probe",
            "1.0.0",
            "centrally retained selected source only; referenced filtered target is not retained",
        )
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let mut target = ReferenceTarget::new_with_source(context.source);
        target.source = Identity::Entry {
            containers: Vec::new(),
            entry: self.target.clone(),
        };
        if matches!(self.mode, ProbeMode::ForeignRoot) {
            target.root.hex = "00".repeat(32);
        }
        let candidate = ReferenceCandidate::new(
            target,
            CandidateState::NotSelected {
                reason: NotSelectedReason::Filter,
            },
        );
        let (candidates, inventory) = if matches!(self.mode, ProbeMode::Incomplete) {
            (
                vec![candidate.clone(), candidate],
                CandidateInventory::Unavailable(ReferenceUnavailable::IncompleteInventory),
            )
        } else {
            (vec![candidate], CandidateInventory::Complete)
        };
        let reference = writer.resolve_reference(
            OriginalIdentifier::try_new_with_text("synthetic.reference", "42")?,
            candidates,
            inventory,
        )?;
        writer.record(
            &Subresource::new("reference", vec![0]),
            Status::Complete,
            &reference,
            &[],
        )?;
        Ok(Projection::Written)
    }
}

fn probe_archive(
    root: &std::path::Path,
    target_path: &str,
    mode: ProbeMode,
) -> Result<(Archive, ProviderRegistry), Box<dyn std::error::Error>> {
    let path = root.join("probe.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("selected.bin", b"selected"))
        .entry(TestEntry::stored(target_path, b"not read"))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = archive.entries().get(1).ok_or("filtered target metadata")?;
    let target = ContainerIdentity::try_new_with_entry(&archive, entry)?;
    let providers = ProviderRegistry::try_new(vec![Box::new(ReferenceProbe { target, mode })])?;
    Ok((archive, providers))
}

fn selected_options() -> Result<ExportOptions, Box<dyn std::error::Error>> {
    Ok(ExportOptions {
        include: Some(Filter::parse("selected.bin")?),
        ..ExportOptions::default()
    })
}

#[test]
fn filtered_target_is_identified_without_reading_or_preserving_it() -> TestResult {
    let folder = tempfile::tempdir()?;
    let (archive, registry) = probe_archive(folder.path(), "filtered.bin", ProbeMode::Filtered)?;
    let output = folder.path().join("media");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &selected_options()?,
        &registry,
    )?)?;
    let rows = support::records(&output, &index, "material.jsonl")?;
    let reference = &rows.first().ok_or("reference record")?.data;
    assert_eq!(
        reference
            .pointer("/resolution/status")
            .and_then(serde_json::Value::as_str),
        Some("not-selected")
    );
    assert_eq!(
        reference
            .pointer("/resolution/reason")
            .and_then(serde_json::Value::as_str),
        Some("filter")
    );
    assert_eq!(
        reference
            .pointer("/resolution/target/source/entry/entry_index")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        (index.source_entries.written, index.source_entries.skipped),
        (1, 1)
    );
    assert_eq!(
        fs::read_dir(output.join(index.artifact_base).join("raw"))?.count(),
        1
    );
    Ok(())
}

#[test]
fn incomplete_actual_candidates_remain_unavailable_and_keep_duplicate_witnesses() -> TestResult {
    let folder = tempfile::tempdir()?;
    let (archive, registry) = probe_archive(folder.path(), "filtered.bin", ProbeMode::Incomplete)?;
    let output = folder.path().join("media");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &selected_options()?,
        &registry,
    )?)?;
    let rows = support::records(&output, &index, "material.jsonl")?;
    let reference = &rows.first().ok_or("reference record")?.data;
    assert_eq!(
        reference
            .pointer("/resolution/status")
            .and_then(serde_json::Value::as_str),
        Some("unavailable")
    );
    assert_eq!(
        reference
            .pointer("/resolution/reason/type")
            .and_then(serde_json::Value::as_str),
        Some("incomplete-inventory")
    );
    assert_eq!(
        reference
            .get("candidates_complete")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        reference
            .get("observed_candidates")
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        reference
            .pointer("/resolution/known_candidates")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    Ok(())
}

#[test]
fn oversized_candidate_metadata_is_unavailable_with_honest_observed_count() -> TestResult {
    let folder = tempfile::tempdir()?;
    let (archive, registry) = probe_archive(folder.path(), &"x".repeat(8192), ProbeMode::Filtered)?;
    let output = folder.path().join("media");
    let mut options = selected_options()?;
    options.limits.record_bytes = 4096;
    let index = support::published(p4kx_media::export_with(
        &archive, &output, &options, &registry,
    )?)?;
    let rows = support::records(&output, &index, "material.jsonl")?;
    let reference = &rows.first().ok_or("reference record")?.data;
    assert_eq!(
        reference
            .pointer("/resolution/reason/type")
            .and_then(serde_json::Value::as_str),
        Some("candidate-bytes")
    );
    assert_eq!(
        reference
            .get("observed_candidates")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        reference
            .get("candidates_complete")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        reference
            .pointer("/resolution/known_candidates")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    Ok(())
}

#[test]
fn a_foreign_root_candidate_cannot_replace_a_complete_generation() -> TestResult {
    let folder = tempfile::tempdir()?;
    let (archive, registry) = probe_archive(folder.path(), "filtered.bin", ProbeMode::ForeignRoot)?;
    let output = folder.path().join("media");
    p4kx_media::export(&archive, &output, &ExportOptions::default())?;
    let previous = fs::read(output.join("media-index.json"))?;
    let mut options = selected_options()?;
    options.conflict = ConflictPolicy::Overwrite;
    assert!(p4kx_media::export_with(&archive, &output, &options, &registry).is_err());
    assert_eq!(fs::read(output.join("media-index.json"))?, previous);
    Ok(())
}

#[test]
fn the_reference_query_budget_applies_across_all_provider_callbacks() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let mut options = ExportOptions::default();
    options.limits.references = 1;
    let output = folder.path().join("media");
    let error = p4kx_media::export_with(&archive, &output, &options, &registry()?)
        .err()
        .ok_or("reference budget unexpectedly succeeded")?;
    let MediaError::Source { cause, .. } = error else {
        return Err("query failure source context".into());
    };
    assert!(matches!(
        *cause,
        MediaError::Limit {
            budget: "references",
            limit: 1
        }
    ));
    assert!(!output.join("media-index.json").exists());
    Ok(())
}
