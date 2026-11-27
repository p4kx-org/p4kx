// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::starmap::{
    ContainerRootStatus, ContainerSystemRoot, ContainerSystemRootInventory, PhysicalEntryRef,
    read_container_system_roots_with_provenance_mode,
};
use p4kx_export::{CatalogError, HealthText};
use p4kx_extraction::{Archive, CancelToken, EntryIssue, OpenOptions, SourceLocation};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::export_health_recovery::remove_directory_record_and_end;
use crate::health_origin_fixture::TestResult;

const ALPHA_FIRST: &str = r"Data\ObjectContainers\PU\system\alpha\asystem.socpak";
const ALPHA_LAST: &str = r"Data\ObjectContainers\PU\system\alpha\zsystem.socpak";
const BETA_PREFERRED: &str = r"Data\ObjectContainers\PU\system\beta\betasystem.socpak";
const BETA_ALIAS: &str = "DATA/objectcontainers/pu/system/BETA/BETASYSTEM.SOCPAK";
const BETA_ALTERNATIVE: &str = r"Data\ObjectContainers\PU\system\beta\asystem.socpak";
const ZETA: &str = r"Data\ObjectContainers\PU\system\zeta\zetasystem.socpak";
const ZETA_ALTERNATIVE: &str = r"Data\ObjectContainers\PU\system\zeta\asystem.socpak";
const NESTED: &str = r"Data\ObjectContainers\PU\system\omega\other\omegasystem.socpak";

#[test]
fn system_root_inventory_preserves_preferred_fallback_and_ambiguous_winning_paths() -> TestResult {
    let paths = [
        ZETA,
        ALPHA_LAST,
        BETA_ALTERNATIVE,
        ALPHA_FIRST,
        BETA_PREFERRED,
        BETA_ALIAS,
        NESTED,
        ZETA_ALTERNATIVE,
    ];
    let (_folder, archive) = open(&paths, false)?;
    let survey =
        read_container_system_roots_with_provenance_mode(&archive, full_proof::MODE, None)?;
    assert!(
        survey.directory_complete(),
        "the complete physical directory was observed"
    );
    assert!(
        !survey.is_complete(),
        "the duplicate preferred path prevents unique root admission"
    );
    let [alpha, beta, zeta] = survey.roots.as_slice() else {
        return Err("only the three direct canonical system paths are roots".into());
    };
    verify_root(
        alpha,
        "alpha",
        ContainerRootStatus::Selected,
        &[index(&archive, ALPHA_FIRST)?],
    )?;
    verify_root(
        beta,
        "beta",
        ContainerRootStatus::Ambiguous,
        &[
            index(&archive, BETA_PREFERRED)?,
            index(&archive, BETA_ALIAS)?,
        ],
    )?;
    verify_root(
        zeta,
        "zeta",
        ContainerRootStatus::Selected,
        &[index(&archive, ZETA)?],
    )?;
    verify_directory_evidence(&survey, archive.entries().len())?;
    Ok(())
}

#[test]
fn system_root_inventory_preserves_observations_without_complete_recovered_membership() -> TestResult
{
    let (_folder, archive) = open(&[ZETA, ALPHA_FIRST], true)?;
    assert!(
        archive
            .issues()
            .iter()
            .any(|(_, issue)| matches!(issue, EntryIssue::Recovered)),
        "the original root directory was recovered"
    );
    let survey =
        read_container_system_roots_with_provenance_mode(&archive, full_proof::MODE, None)?;
    assert!(
        !survey.directory_complete(),
        "recovering a prefix cannot prove directory membership"
    );
    assert!(
        !survey.is_complete(),
        "the query cannot assert that all root choices are known"
    );
    let [observed] = survey.roots.as_slice() else {
        return Err("only the recovered physical zeta entry remains observable".into());
    };
    verify_root(
        observed,
        "zeta",
        ContainerRootStatus::Selected,
        &[index(&archive, ZETA)?],
    )?;
    assert!(
        !observed.status.origin().source_set_complete(),
        "an observed unique candidate retains incomplete directory evidence"
    );
    Ok(())
}

#[test]
fn system_root_inventory_distinguishes_complete_empty_from_cancellation() -> TestResult {
    let (_folder, archive) = open(&[NESTED], false)?;
    let survey =
        read_container_system_roots_with_provenance_mode(&archive, full_proof::MODE, None)?;
    assert!(
        survey.roots.is_empty(),
        "a nested system descriptor is never a root"
    );
    assert!(survey.is_complete(), "known empty root survey");
    verify_directory_evidence(&survey, archive.entries().len())?;
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(
        matches!(
            read_container_system_roots_with_provenance_mode(
                &archive,
                full_proof::MODE,
                Some(&cancel)
            ),
            Err(CatalogError::Cancelled)
        ),
        "a cancelled survey cannot establish a complete empty result"
    );
    Ok(())
}

fn verify_root(
    root: &ContainerSystemRoot,
    expected_system: &str,
    expected_status: ContainerRootStatus,
    expected_indices: &[u32],
) -> TestResult {
    assert_eq!(
        root.system.value().map(HealthText::as_str),
        Some(expected_system),
        "canonical system folders are emitted in Normal order"
    );
    assert_eq!(
        root.status.value(),
        Some(&expected_status),
        "classification of the winning path"
    );
    let observed = root
        .candidates
        .iter()
        .map(|candidate| {
            candidate
                .value()
                .map(PhysicalEntryRef::entry_index)
                .ok_or("bound candidate Entry")
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        observed.as_slice(),
        expected_indices,
        "one candidate per physical directory occurrence of the winning path"
    );
    let selected = match expected_indices {
        [entry] => Some(*entry),
        _ => None,
    };
    assert_eq!(
        root.selected_entry().map(PhysicalEntryRef::entry_index),
        selected,
        "ambiguity cannot silently select a fallback"
    );
    Ok(())
}

fn verify_directory_evidence(survey: &ContainerSystemRootInventory, count: usize) -> TestResult {
    let expected_end = u64::try_from(count)?;
    let directory = survey.directory_origin.source_uses_refs();
    let inventory = survey.source_inventory.select_uses(directory)?;
    let location = inventory
        .iter_uses()
        .next()
        .ok_or("root path survey use")?
        .location();
    assert!(
        matches!(
            location,
            SourceLocation::ArchiveDirectoryPaths { start_entry_index, end_entry_index }
                if start_entry_index.get() == 0 && end_entry_index.get() == expected_end
        ),
        "the original directory path range covers all physical Entries"
    );
    for root in &survey.roots {
        assert!(
            root.status.origin().source_set_complete(),
            "classification has complete input"
        );
        assert!(
            directory
                .iter()
                .all(|id| root.status.origin().source_uses_refs().contains(id)),
            "root selection binds the actual directory range"
        );
        for candidate in &root.candidates {
            let references = candidate.origin().source_uses_refs();
            let provenance = survey.source_inventory.select_uses(references)?;
            assert_eq!(
                provenance.iter_uses().len(),
                2,
                "physical path and directory ordinal"
            );
        }
    }
    Ok(())
}

fn index(archive: &Archive, path: &str) -> TestResult<u32> {
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .ok_or("physical root fixture")?;
    Ok(u32::try_from(entry.index())?)
}

fn open(paths: &[&str], damaged: bool) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let mut archive = ArchiveBuilder::new();
    for &entry in paths {
        archive = archive.entry(TestEntry::stored(
            entry,
            b"not a valid descriptor".as_slice(),
        ));
    }
    let mut bytes = archive.build();
    if damaged {
        remove_directory_record_and_end(&mut bytes)?;
    }
    std::fs::write(&path, bytes)?;
    Ok((
        folder,
        Archive::open_with(&path, OpenOptions::new().recover(damaged))?,
    ))
}
