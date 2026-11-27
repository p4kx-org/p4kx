// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fmt;

use p4kx_extraction::SourceValueState;

use super::export::{
    EntryOperation, ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage,
    ExportHealth, ExportSourceRole, FailedExportEntry, Kind,
};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct EntryGroup {
    kind: &'static str,
    role: ExportSourceRole,
    operation: EntryOperation,
    code: ExportDiagnosticCode,
    stage: ExportDiagnosticStage,
    status: SourceValueState,
}

impl EntryGroup {
    const fn new(entry: &FailedExportEntry) -> Self {
        Self {
            kind: entry.kind().name(),
            role: entry.role(),
            operation: entry.operation(),
            code: entry.code(),
            stage: entry.stage(),
            status: entry.entry_index().state(),
        }
    }
}

pub(super) fn report(
    channel: &str,
    change: &str,
    health: &ExportHealth,
    mut emit: impl FnMut(fmt::Arguments<'_>),
) {
    emit(format_args!(
        "catalog sizing refusal input={channel} change={change} read={:?} projection={:?}",
        health.read, health.projection,
    ));
    for row in health
        .diagnostics()
        .iter()
        .filter(|row| row.severity != ExportDiagnosticSeverity::Information)
    {
        let kind = row.kind.map_or("catalog", Kind::name);
        let warning = row.warning_code.as_deref().unwrap_or("-");
        emit(format_args!(
            "catalog sizing refusal diagnostic input={channel} change={change} kind={kind} stage={:?} severity={:?} code={:?} role={:?} warning={warning} count={} unit={:?}",
            row.stage, row.severity, row.code, row.source_role, row.count, row.count_unit,
        ));
    }

    let mut groups = BTreeMap::<EntryGroup, usize>::new();
    for entry in health.failed_entries() {
        *groups.entry(EntryGroup::new(entry)).or_default() += 1;
    }
    for (group, count) in groups {
        emit(format_args!(
            "catalog sizing refusal entry input={channel} change={change} kind={} role={:?} operation={:?} code={:?} stage={:?} entry_status={:?} count={count}",
            group.kind, group.role, group.operation, group.code, group.stage, group.status,
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::io;
    use std::time::UNIX_EPOCH;

    use p4kx_extraction::{Archive, DataForgeEntry};
    use p4kx_testkit::{
        ArchiveBuilder, DataForgeBuilder, TestContainer, TestEntry, TestRecord, TestStruct,
    };

    use super::super::{TestResult, export};
    use super::report;

    #[test]
    fn sizing_refusal_groups_failures_without_exposing_source_identities() -> TestResult {
        let (_directory, archive) = refused_sources()?;
        let entry = DataForgeEntry::read(&archive)?;
        let options = export::CatalogOptions::new(UNIX_EPOCH);
        let output_requested = Cell::new(false);
        let result = export::write_catalog_from(
            &archive,
            &entry,
            options.with_full_provenance(),
            export::Compression::ZSTD,
            || {
                output_requested.set(true);
                Ok(io::sink())
            },
        );
        let Err(export::CatalogError::IncompleteExport(refusal)) = result else {
            return Err("normal publication must refuse the failed operations".into());
        };
        assert!(
            !output_requested.get(),
            "the refusal must precede opening output"
        );

        let mut lines = Vec::new();
        report("LIVE", "123", refusal.health(), |line| {
            lines.push(line.to_string());
        });
        assert_report(&lines);
        Ok(())
    }

    fn assert_report(lines: &[String]) {
        assert!(
            lines
                .iter()
                .all(|line| line.contains("input=LIVE change=123")),
            "each row identifies its measurement input"
        );
        assert!(
            lines.iter().any(|line| line.contains(
                "kind=shopInventories stage=Projection severity=Error code=MalformedSource role=Some(DerivedInput) warning=- count=2 unit=EntryOperations"
            )),
            "the health aggregate counts malformed files and excludes the verified empty file"
        );
        assert!(
            lines.iter().any(|line| line.ends_with(
                "kind=shopInventories role=DerivedInput operation=ShopInventory code=MalformedSource stage=Projection entry_status=Present count=2"
            )),
            "equal failed Entry operations are grouped"
        );
        assert!(
            lines.iter().any(|line| line.ends_with(
                "kind=shopInventories role=DerivedInput operation=ShopInventory code=ArchiveKeyRequired stage=SourceRead entry_status=Present count=1"
            )),
            "key-required payloads retain their physically present Entry status"
        );
        assert!(
            lines.iter().any(|line| line.ends_with(
                "kind=lootContainers stage=DerivedProjection severity=Warning code=VerifiedMissingReference role=Some(DerivedInput) warning=- count=1 unit=EntryOperations"
            )),
            "the proved absent target is reported as a warning"
        );
        assert!(
            lines.iter().all(|line| {
                !line.starts_with("catalog sizing refusal entry")
                    || !line.contains("kind=lootContainers")
            }),
            "the verified absent target contributes no failed Entry operation"
        );
        assert!(
            lines.iter().all(|line| !line.contains("private-")),
            "authored paths and record identities never reach the measurement log"
        );
    }

    fn refused_sources() -> TestResult<(tempfile::TempDir, Archive)> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("Data.p4k");
        let dataforge = DataForgeBuilder::new()
            .version(8)
            .structure(TestStruct::new("StarMapObject"))
            .record(TestRecord::new(
                0x5712,
                "StarMapObject",
                "StarMapObject.private-map",
                "libs/foundry/records/starmap/pu/system/alpha/private-map.xml",
            ))
            .build();
        let system = TestContainer::new()
            .entry(TestEntry::stored("alphasystem.xml", "<ObjectContainer/>"))
            .entry(TestEntry::stored(
                "entdata/private-entity.entxml",
                r#"<Entity objectContainer="Data/FixtureContainers/private-missing.socpak"/>"#,
            ))
            .build();
        ArchiveBuilder::new()
            .entry(TestEntry::stored("Data/Game2.dcb", dataforge))
            .entry(TestEntry::stored(
                "Data/ObjectContainers/PU/system/alpha/alphasystem.socpak",
                system,
            ))
            .entry(TestEntry::stored(
                "Data/Scripts/ShopInventories/private-empty.json",
                "",
            ))
            .entry(TestEntry::stored(
                "Data/Scripts/ShopInventories/private-malformed.json",
                "{",
            ))
            .entry(TestEntry::stored(
                "Data/Scripts/ShopInventories/private-invalid.json",
                "]",
            ))
            .entry(
                TestEntry::stored(
                    "Data/Scripts/ShopInventories/private-encrypted.json",
                    r#"{"Inventory":[]}"#,
                )
                .encrypted(),
            )
            .write_to(&path)?;
        Ok((directory, Archive::open(path)?))
    }
}
