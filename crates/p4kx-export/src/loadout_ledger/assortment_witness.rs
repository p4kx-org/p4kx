// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write as _};
use std::path::PathBuf;

use p4kx_assembly::{ArchiveFiles, EntityNode, LoadoutError, LoadoutResolver};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Guid, Record, TreeBuilder,
};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::{Code, Frame, Issues, Severity, category};
use crate::Kind;
use crate::entity_selection::{Selected, select};
use crate::guid_text::CatalogGuid;
use crate::loadout::carries_ports;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

const WITNESSED_LIVE_CHANGE: &str = "13165946";
const PRIVATE_RECEIPT: &str = "P4KX_ASSORTMENT_WITNESS_FILE";

struct Root<'source> {
    kind: Kind,
    record: Record<'source>,
}

#[derive(Default)]
struct Census {
    roots: usize,
    header_failures: usize,
    eligibility_failures: usize,
    resolution_failures: usize,
    matches: Vec<Match>,
}

struct Match {
    kind: Kind,
    root_index: u32,
    root_guid: Guid,
    target_index: Option<u32>,
    target_guid: Guid,
    reason: String,
}

fn roots<'source>(forge: &'source DataForge<'source>, census: &mut Census) -> Vec<Root<'source>> {
    let mut candidates = Vec::new();
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        match select(&record, None) {
            Ok(Some(Selected::Entity(Kind::Ships))) => candidates.push(Root {
                kind: Kind::Ships,
                record,
            }),
            Ok(Some(Selected::Entity(Kind::Items))) => match trees.build(&record) {
                Ok(tree) if carries_ports(&tree) => candidates.push(Root {
                    kind: Kind::Items,
                    record,
                }),
                Ok(_) => {}
                Err(_) => census.eligibility_failures += 1,
            },
            Ok(_) => {}
            Err(_) => census.header_failures += 1,
        }
    }
    candidates.sort_by_cached_key(|root| {
        (
            CatalogGuid(root.record.guid()).text(),
            root.kind != Kind::Ships,
        )
    });
    candidates
}

fn run<'source>(
    forge: &'source DataForge<'source>,
    mut resolver: LoadoutResolver<'source>,
) -> Census {
    let mut census = Census::default();
    for root in roots(forge, &mut census) {
        census.roots += 1;
        match resolver.resolve(&root.record) {
            Ok(node) => record_matches(forge, &root, &node, &mut census.matches),
            Err(_) => census.resolution_failures += 1,
        }
    }
    census
}

fn record_matches(
    forge: &DataForge<'_>,
    root: &Root<'_>,
    node: &EntityNode,
    matches: &mut Vec<Match>,
) {
    let issues = Issues {
        stack: vec![Frame::Node(node)],
    };
    for issue in issues {
        let LoadoutError::Assortment { guid, reason } = issue.error else {
            continue;
        };
        if category(issue.error) != (Code::UnsupportedValue, Severity::Warning) {
            continue;
        }
        let target_guid = guid.guid();
        matches.push(Match {
            kind: root.kind,
            root_index: root.record.index(),
            root_guid: root.record.guid(),
            target_index: forge
                .record_by_guid(target_guid)
                .map(|target| target.index()),
            target_guid,
            reason: reason.clone(),
        });
    }
}

fn archive_from_live() -> Result<Archive> {
    let path = real_archive_path()?;
    if p4kx_channels::channel_name(&path)
        .is_none_or(|channel| !channel.eq_ignore_ascii_case("LIVE"))
    {
        return Err("expected the installed LIVE Archive".into());
    }
    let archive = Archive::open_with_key(&path, Some(real_archive_key()?))
        .map_err(|_| "LIVE Archive open failed")?;
    let build = archive.build().ok_or("LIVE Build identity missing")?;
    if build.change_number() != WITNESSED_LIVE_CHANGE
        || [build.branch(), build.version(), build.build_id()]
            .iter()
            .any(|value| value.trim().is_empty())
        || archive.file_modified().is_none()
    {
        return Err("LIVE Archive does not identify the measured Build".into());
    }
    Ok(archive)
}

fn write_private_receipt(
    path: PathBuf,
    archive: &Archive,
    entry: &DataForgeEntry,
    census: &Census,
) -> Result {
    let build = archive.build().ok_or("LIVE Build identity missing")?;
    let mut writer =
        BufWriter::new(File::create_new(path).map_err(|_| "private receipt unavailable")?);
    writeln!(
        writer,
        "branch={:?} version={:?} change={} build_id={:?} archive_bytes={} modified={:?}",
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
        archive.file_len(),
        archive.file_modified()
    )?;
    let DataForgeOrigin::Archive(origin) = entry.origin() else {
        return Err("expected Archive-bound DataForge origin".into());
    };
    writeln!(writer, "dataforge_entry={}", origin.entry().ordinal())?;
    for finding in &census.matches {
        writeln!(
            writer,
            "kind={} root_index={} root_guid={} target_index={:?} target_guid={} reason={:?}",
            finding.kind.name(),
            finding.root_index,
            finding.root_guid,
            finding.target_index,
            finding.target_guid,
            finding.reason
        )?;
    }
    writer.flush()?;
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::print_stdout,
    reason = "the private witness reports aggregate counts on stdout"
)]
fn installed_live_empty_xml_alternatives_leave_complete_default_resolution() -> Result {
    let receipt = env::var_os(PRIVATE_RECEIPT)
        .map(PathBuf::from)
        .ok_or("set P4KX_ASSORTMENT_WITNESS_FILE to a new private scratch file")?;
    if !receipt.is_absolute() || receipt.try_exists()? {
        return Err("expected an unused absolute private receipt path".into());
    }
    let archive = archive_from_live()?;
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|_| "LIVE Archive changed before the witness")?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "LIVE DataForge read failed")?;
    if !entry.origin().matches_archive(&archive) {
        return Err("LIVE DataForge identity does not match the Archive".into());
    }
    let forge = entry.open().map_err(|_| "LIVE DataForge parse failed")?;
    let files = ArchiveFiles::new(&archive);
    let resolver = LoadoutResolver::new(&forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let census = run(&forge, resolver);
    watch
        .check()
        .map_err(|_| "LIVE Archive changed during the witness")?;
    write_private_receipt(receipt, &archive, &entry, &census)?;
    println!(
        "loadout assortment witness: roots={} unusable_headers={} unreadable_eligibility={} resolution_failures={} matches={}",
        census.roots,
        census.header_failures,
        census.eligibility_failures,
        census.resolution_failures,
        census.matches.len()
    );
    assert!(
        census.roots > 0,
        "eligible installed default roots were exercised"
    );
    assert_eq!(
        [
            census.header_failures,
            census.eligibility_failures,
            census.resolution_failures,
            census.matches.len()
        ],
        [0; 4],
        "complete installed resolution without the empty-path warning"
    );
    Ok(())
}

#[path = "assortment_witness_tests.rs"]
mod cases;
