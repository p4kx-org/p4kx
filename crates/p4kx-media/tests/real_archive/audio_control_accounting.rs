// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reconciles ATL exports with original scoped audio control definitions.

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use p4kx_assembly::audio::AudioIndex;
use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::sound::wwise_path;
use p4kx_extraction::{Archive, ArchiveWatch, Exclusion, Filter};
use p4kx_media::{
    ContainerIdentity, ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord, Status,
};
use p4kx_testkit::{real_archive_key, real_archive_path};

#[path = "privacy.rs"]
mod privacy;
#[path = "audio_control_support.rs"]
mod support;

use support::require;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[path = "audio_control_trigger_oracle.rs"]
mod trigger_oracle;

use self::trigger_oracle::Triggers;

#[derive(Debug, Default, PartialEq, Eq)]
struct Census {
    documents: u64,
    elements: u64,
    declarations: u64,
    definitions: u64,
    edges: u64,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn original_atl_source_counts_reconcile_with_media_and_assembly()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_configured())
}

fn verify_configured() -> TestResult {
    verify_archive(&real_archive_path()?)?;
    if let Some(path) = p4kx_testkit::real_archive_b_path() {
        verify_archive(&path)?;
    }
    Ok(())
}

fn verify_archive(path: &Path) -> TestResult {
    let archive = Archive::open_with_key(path, Some(real_archive_key()?))?;
    verify_root(&archive)
}

fn verify_root(archive: &Archive) -> TestResult {
    let watch = ArchiveWatch::new(archive);
    let (expected, triggers) = source_counts(archive, &watch)?;
    require(!expected.is_empty(), "root definitions present")?;
    let scratch = tempfile::tempdir()?;
    let options = ExportOptions {
        kinds: vec![MediaKind::AudioControl],
        include: Some(Filter::parse(r"Data\Libs\GameAudio\*.xml")?),
        exclude: Exclusion::parse(["*.pak", "*.socpak"])?,
        ..ExportOptions::default()
    };
    let ExportOutcome::Published { index, .. } =
        p4kx_media::export(archive, scratch.path(), &options)?
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
        actual == expected,
        "physical ATL occurrence and edge counts agree",
    )?;
    triggers.verify(archive, &AudioIndex::build(archive))?;
    watch.check()?;
    scratch.close()?;
    Ok(())
}

fn source_counts(
    archive: &Archive,
    watch: &ArchiveWatch,
) -> TestResult<(BTreeMap<usize, Census>, Triggers)> {
    let mut counts = BTreeMap::new();
    let mut triggers = Triggers::default();
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
            triggers.observe(entry.path(), position, root);
        }
        counts.insert(position, census);
    }
    Ok((counts, triggers))
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
        let census = counts.entry(entry.entry_index).or_default();
        match row.subresource.namespace.as_str() {
            "audio-control-document" => census.documents += 1,
            "audio-control-element" => census.elements += 1,
            "audio-control-declaration" => census.declarations += 1,
            "audio-control-definition" => census.definitions += 1,
            "audio-control-reference" => census.edges += 1,
            _ => {}
        }
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
          <AudioPreloads><ATLPreloadRequest atl_name="source"><ATLPreload atl_name="missing"/></ATLPreloadRequest></AudioPreloads>
        </ATLConfig>"#))
        .entry(TestEntry::stored("Data/Libs/GameAudio/A.xml", first.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)).encrypted())
        .entry(TestEntry::stored("Data/Libs/GameAudio/settings.xml", b"<Settings><NotAControl/></Settings>"))
        .entry(TestEntry::stored("Data/Objects/malformed.socpak", b"excluded nested archive"))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    verify_root(&archive)
}
