// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use p4kx_assembly::audio::AudioIndex;
use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::sound::wwise_path;
use p4kx_extraction::{Archive, Filter};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[derive(Debug, Default, PartialEq, Eq)]
struct Census {
    documents: u64,
    elements: u64,
    declarations: u64,
    edges: u64,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn original_atl_source_counts_reconcile_with_media_and_assembly() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "installed ATL Archive open failed")?;
    let (expected, trigger_count) = source_counts(&archive)?;
    assert!(
        expected.documents > 0,
        "the installed Archive contains ATL definitions"
    );
    let scratch = tempfile::tempdir()?;
    let options = ExportOptions {
        kinds: vec![MediaKind::AudioControl],
        include: Some(Filter::parse(r"Data\Libs\GameAudio\*.xml")?),
        ..ExportOptions::default()
    };
    let ExportOutcome::Published { index, .. } =
        p4kx_media::export(&archive, scratch.path(), &options)
            .map_err(|_| "installed ATL Media export failed")?
    else {
        return Err("installed ATL census requires a complete generation".into());
    };
    let stream = scratch
        .path()
        .join(&index.artifact_base)
        .join("audio-control.jsonl");
    assert_eq!(
        media_counts(&stream)?,
        expected,
        "root ATL source and export occurrence counts agree"
    );
    assert_eq!(
        AudioIndex::build(&archive).triggers().len() as u64,
        trigger_count,
        "installed original trigger declarations reconcile with the compatible Assembly index"
    );
    Ok(())
}

fn source_counts(archive: &Archive) -> TestResult<(Census, u64)> {
    let mut census = Census::default();
    let mut trigger_count = 0;
    for entry in archive
        .entries()
        .iter()
        .filter(|entry| wwise_path::is_atl_definition(entry.path()))
    {
        let mut bytes = Vec::new();
        archive
            .open_entry(entry)
            .map_err(|_| "installed ATL Entry open failed")?
            .read_to_end(&mut bytes)
            .map_err(|_| "installed ATL Entry read failed")?;
        let source = SourceDocument::parse(&bytes, SourceReadOptions::default())
            .map_err(|_| "installed ATL source parse failed")?;
        let root = source.root();
        census.documents += 1;
        let typed = root.name() == "ATLConfig";
        tally(root, typed, &mut census)?;
        if typed {
            trigger_count += root
                .elements()
                .filter(|list| list.name() == "AudioTriggers")
                .flat_map(SourceElement::elements)
                .filter(|element| {
                    element.name() == "ATLTrigger"
                        && element
                            .attribute("atl_name")
                            .is_some_and(|name| !name.is_empty())
                })
                .count() as u64;
        }
    }
    Ok((census, trigger_count))
}

fn tally(root: SourceElement<'_>, typed: bool, census: &mut Census) -> TestResult {
    let mut pending = vec![root];
    while let Some(element) = pending.pop() {
        census.elements += 1;
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

fn media_counts(path: &Path) -> TestResult<Census> {
    let mut census = Census::default();
    for line in BufReader::new(File::open(path)?).lines() {
        let row: MediaRecord =
            serde_json::from_str(&line?).map_err(|_| "installed ATL Media record is invalid")?;
        if !matches!(row.source.identity, Identity::Entry { ref containers, .. } if containers.is_empty())
        {
            continue;
        }
        match row.subresource.namespace.as_str() {
            "audio-control-document" => census.documents += 1,
            "audio-control-element" => census.elements += 1,
            "audio-control-declaration" => census.declarations += 1,
            "audio-control-reference" => census.edges += 1,
            _ => {}
        }
    }
    Ok(census)
}
