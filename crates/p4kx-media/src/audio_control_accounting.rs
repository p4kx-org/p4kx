// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Compares independent ATL source counts with the Media projection and Assembly triggers.

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use p4kx_assembly::audio::AtlDefinitions;
use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::sound::wwise_path;
use p4kx_extraction::{Archive, ArchiveWatch, Checksum, Exclusion, Filter};
use sha2::{Digest as _, Sha256};

use crate::audio_control_projection::{self, Failure};
use crate::media_writer::Finished;
use crate::origin_test_support::real_archive_privacy as privacy;
use crate::{
    Artifact, ContainerIdentity, ExportOptions, ExportOutcome, Identity, Index, MediaError,
    MediaKind, MediaRecord, Status,
};

#[path = "audio_control_accounting_support.rs"]
mod support;

use support::{require, require_stream_phases};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[path = "audio_control_accounting_installed.rs"]
mod installed;
#[path = "audio_control_accounting_observer.rs"]
mod observer;
#[path = "audio_control_production_harness.rs"]
mod production_harness;

#[path = "audio_control_trigger_oracle.rs"]
mod trigger_oracle;

use self::production_harness::StreamPhase;
use self::trigger_oracle::Triggers;

#[derive(Debug, Default, PartialEq, Eq)]
struct Census {
    documents: u64,
    elements: u64,
    declarations: u64,
    definitions: u64,
    edges: u64,
}

fn verify_synthetic_export(archive: &Archive) -> TestResult {
    let watch = ArchiveWatch::new(archive);
    let original = source_counts(archive, &watch)?;
    require(!original.rows.is_empty(), "root definitions present")?;
    let mut phases = Vec::new();
    let streamed = verify_production_stream(archive, &original, |phase| {
        phases.push(phase);
        Ok(())
    })?;
    require_stream_phases(&phases)?;
    let scratch = tempfile::tempdir()?;
    let options = ExportOptions {
        kinds: vec![MediaKind::AudioControl],
        include: Some(Filter::parse(r"Data\Libs\GameAudio\*.xml")?),
        exclude: Exclusion::parse(["*.pak", "*.socpak"])?,
        ..ExportOptions::default()
    };
    let ExportOutcome::Published { index, .. } = crate::export(archive, scratch.path(), &options)?
    else {
        return Err("root audio-control generation incomplete".into());
    };

    let base = scratch.path().join(&index.artifact_base);
    let stream = index
        .kinds
        .iter()
        .find(|kind| kind.provider.kind == MediaKind::AudioControl)
        .ok_or("root audio control stream")?;
    let actual = media_counts(archive, &base, &base.join(&stream.artifact.path))?;
    require(
        actual == original.rows,
        "physical ATL occurrence and edge counts agree",
    )?;

    verify_receipts(&streamed, &index, &stream.artifact)?;
    original
        .triggers
        .verify(archive, &AtlDefinitions::build(archive))?;
    watch.check()?;
    scratch.close()?;
    Ok(())
}

fn verify_receipts(observed: &Finished, index: &Index, records: &Artifact) -> TestResult {
    let observed_records = observed
        .kinds
        .iter()
        .find(|kind| kind.provider.kind == MediaKind::AudioControl)
        .ok_or("observed ATL Media stream")?;
    let published = index
        .field_origins
        .as_ref()
        .ok_or("published origin receipts")?;
    for (streamed, file, message) in [
        (&observed_records.artifact, records, "ATL record bytes"),
        (
            &observed.sources,
            &index.source_manifest,
            "source manifest bytes",
        ),
        (
            &observed.artifacts,
            &index.artifact_manifest,
            "artifact manifest bytes",
        ),
        (
            &observed.origins.fields,
            &published.fields,
            "field origin bytes",
        ),
        (
            &observed.origins.sources,
            &published.sources,
            "physical source bytes",
        ),
        (&observed.origins.uses, &published.uses, "source use bytes"),
        (
            &observed.origins.sets,
            &published.sets,
            "source use set bytes",
        ),
    ] {
        require(
            streamed.path == file.path
                && streamed.size == file.size
                && streamed.digest == file.digest,
            message,
        )?;
    }
    Ok(())
}

impl Census {
    fn observe(&mut self, namespace: &str) {
        use crate::audio_control_payload as wire;
        match namespace {
            wire::DOCUMENT => self.documents += 1,
            wire::ELEMENT => self.elements += 1,
            wire::DECLARATION => self.declarations += 1,
            wire::DEFINITION => self.definitions += 1,
            wire::REFERENCE => self.edges += 1,
            _ => {}
        }
    }
}

struct SourceCensus {
    rows: BTreeMap<usize, Census>,
    digests: BTreeMap<usize, String>,
    triggers: Triggers,
}

fn verify_production_stream(
    archive: &Archive,
    original: &SourceCensus,
    mut report: impl FnMut(StreamPhase) -> TestResult,
) -> TestResult<Finished> {
    let observer = observer::Observer::new(archive, &original.digests);
    let streamed = production_harness::observe(archive, &observer, &mut report)?;
    require(
        observer.counts()? == original.rows,
        "production ATL serialised rows match the original occurrence counts",
    )?;
    Ok(streamed)
}

fn source_counts(archive: &Archive, watch: &ArchiveWatch) -> TestResult<SourceCensus> {
    let mut result = SourceCensus {
        rows: BTreeMap::new(),
        digests: BTreeMap::new(),
        triggers: Triggers::default(),
    };
    for (position, entry) in archive.entries().iter().enumerate() {
        watch.poll()?;
        if !wwise_path::is_atl_definition(entry.path()) {
            continue;
        }
        let mut bytes = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut bytes)?;
        let source = SourceDocument::parse(&bytes, SourceReadOptions::default())?;
        let root = source.root();
        let typed = root.name() == "ATLConfig";
        let mut census = Census {
            documents: 1,
            ..Default::default()
        };
        tally(root, typed, &mut census)?;
        if typed {
            result.triggers.observe(entry.path(), position, root);
        }
        result.rows.insert(position, census);
        result.digests.insert(
            position,
            Checksum::Sha256(Sha256::digest(&bytes).into()).hex(),
        );
    }
    Ok(result)
}

fn tally(root: SourceElement<'_>, typed: bool, census: &mut Census) -> TestResult {
    let mut pending = vec![root];
    while let Some(element) = pending.pop() {
        census.elements += 1;
        census.definitions += u64::from(typed && is_definition(element.name()));
        let syntax = element
            .syntax()
            .map_err(|_| "installed ATL element syntax unavailable")?;
        for declaration in syntax.declarations() {
            let declaration =
                declaration.map_err(|_| "installed ATL declaration syntax unavailable")?;
            census.declarations += 1;
            if typed && is_link(element.name(), declaration.qualified_name()) {
                census.edges += 1;
            }
        }
        pending.extend(element.elements());
    }
    Ok(())
}

fn is_link(element: &str, attribute: &str) -> bool {
    matches!(
        (element, attribute),
        ("ATLPreload" | "ATLRTPCDependee", "atl_name")
            | ("Platform", "atl_config_group_name")
            | ("WwiseExternalSource", "wwise_filename")
            | (
                "WwiseEvent"
                    | "WwiseFile"
                    | "WwiseExternalSource"
                    | "WwiseRtpc"
                    | "WwiseSwitch"
                    | "WwiseState"
                    | "WwiseAuxBus"
                    | "WwiseValue",
                "wwise_name"
            )
    )
}

fn is_definition(tag: &str) -> bool {
    matches!(
        tag,
        "ATLConfig"
            | "AudioTriggers"
            | "ATLTrigger"
            | "WwiseEvent"
            | "AudioRTPCs"
            | "AudioRtpcs"
            | "ATLRtpc"
            | "WwiseRtpc"
            | "AudioSwitches"
            | "ATLSwitch"
            | "ATLSwitchState"
            | "WwiseSwitch"
            | "WwiseState"
            | "WwiseValue"
            | "AudioEnvironments"
            | "ATLEnvironment"
            | "WwiseAuxBus"
            | "AudioPreloads"
            | "ATLPreloadRequest"
            | "ATLPreload"
            | "RTPCDependees"
            | "ATLRTPCDependee"
            | "ATLConfigGroup"
            | "ATLPlatforms"
            | "Platform"
            | "Tags"
            | "Tag"
            | "WwiseFile"
            | "AudioExternalSources"
            | "ATLExternalSource"
            | "ATLExternalSourceEntry"
            | "WwiseExternalSource"
    )
}

fn media_counts(
    archive: &Archive,
    base: &Path,
    path: &Path,
) -> TestResult<BTreeMap<usize, Census>> {
    let mut counts = BTreeMap::<usize, Census>::new();
    let mut originals = HashMap::new();
    for line in BufReader::new(File::open(path)?).lines() {
        let row: MediaRecord = serde_json::from_str(&line?)?;
        let Identity::Entry { containers, entry } = &row.source.identity else {
            continue;
        };
        require(containers.is_empty(), "only selected root audio sources")?;
        require(
            row.status == Status::Complete,
            "each selected ATL row is complete",
        )?;
        let physical = archive
            .entries()
            .get(entry.entry_index)
            .ok_or("root ATL physical entry")?;
        if let std::collections::hash_map::Entry::Vacant(slot) = originals.entry(entry.entry_index)
        {
            require(
                *entry == ContainerIdentity::try_new_with_entry(archive, physical)?,
                "original ATL has the same physical identity",
            )?;
            let raw = row
                .source
                .raw
                .as_ref()
                .ok_or("retained original ATL body")?;
            support::verify_original(archive, physical, base, raw)?;
            slot.insert(raw.clone());
        }
        require(
            row.source.raw.as_ref() == originals.get(&entry.entry_index),
            "all ATL rows retain the same original body",
        )?;
        counts
            .entry(entry.entry_index)
            .or_default()
            .observe(&row.subresource.namespace);
    }
    Ok(counts)
}

#[test]
fn root_atl_accounts_for_physical_documents_and_index_precedence() -> TestResult {
    use p4kx_testkit::{
        ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry, test_key,
    };

    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let first = TestElement::new("ATLConfig").child(
        TestElement::new("AudioTriggers").child(
            TestElement::new("ATLTrigger")
                .attribute("atl_name", "winner")
                .child(TestElement::new("WwiseEvent").attribute("wwise_name", "variant"))
                .child(
                    TestElement::new("WwiseEvent")
                        .attribute("wwise_name", "second")
                        .attribute("wwise_localised", "true"),
                ),
        ),
    );
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Libs/GameAudio/z.xml", br#"<ATLConfig>
          <AudioTriggers><ATLTrigger atl_name="WINNER"><WwiseEvent wwise_name="shadow"/></ATLTrigger>
          <ATLTrigger atl_name="one"><WwiseEvent wwise_name="sole"/></ATLTrigger></AudioTriggers>
          <AudioRTPCs><ATLRtpc atl_name="rate"><WwiseRtpc wwise_name="speed"/></ATLRtpc></AudioRTPCs>
          <AudioPreloads><ATLPreloadRequest atl_name="source">
            <ATLPreload atl_name="missing"/><ATLPreload atl_name=""/>
            <RTPCDependees><ATLRTPCDependee atl_name="rate"/></RTPCDependees>
            <ATLPlatforms><Platform atl_config_group_name="desktop"/></ATLPlatforms>
            <ATLConfigGroup atl_name="desktop"><WwiseFile wwise_name="Data/Sounds/wwise/synthetic.bnk"/></ATLConfigGroup>
          </ATLPreloadRequest></AudioPreloads>
          <AudioExternalSources><ATLExternalSource atl_name="external"><ATLExternalSourceEntry>
            <WwiseExternalSource wwise_name="" wwise_filename="sample.wem"/>
          </ATLExternalSourceEntry></ATLExternalSource></AudioExternalSources>
        </ATLConfig>"#))
        .entry(TestEntry::stored("Data/Libs/GameAudio/A.xml", first.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)).encrypted())
        .entry(TestEntry::stored("Data/Libs/GameAudio/settings.xml", b"<Settings><NotAControl/></Settings>"))
        .entry(TestEntry::stored("Data/Objects/malformed.socpak", b"excluded nested archive"))
        .entry(TestEntry::stored("Data/Sounds/wwise/synthetic.bnk", b"directory-only target"))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    verify_synthetic_export(&archive)
}

#[test]
fn atl_census_propagates_cancellation_before_counting_rows() -> TestResult {
    let source = SourceDocument::parse(
        b"<ATLConfig><AudioTriggers/></ATLConfig>",
        SourceReadOptions::default(),
    )?;
    let result = audio_control_projection::census(&source, 0, || Err(MediaError::Cancelled));
    assert!(
        matches!(result, Err(Failure::Fatal(MediaError::Cancelled))),
        "census preserves the production checkpoint error",
    );
    Ok(())
}
