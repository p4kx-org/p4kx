// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{LoadoutFileError, LoadoutFiles};

use super::{
    Arc, Capture, CatalogGuid, DataForge, DataForgeBuilder, Entities, EntityKind,
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportLedger, LoadoutResolver, PortReader,
    ROOT, TestEntity, TestLoadout, TestResult, Wanted,
};

#[derive(Clone, Copy)]
enum Reply {
    Missing,
    Unreadable,
    Malformed,
}

impl LoadoutFiles for Reply {
    fn read(&self, _path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        match self {
            Self::Missing => Err(LoadoutFileError::NotFound),
            Self::Unreadable => Err(LoadoutFileError::Unreadable {
                reason: "synthetic read fault".to_owned(),
            }),
            Self::Malformed => Ok(b"<broken>".to_vec()),
        }
    }
}

const VEHICLE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Synthetic.xml";
const XML: &str = "synthetic/loadout.xml";

fn read(files: Reply, vehicle: bool) -> Result<ExportLedger, Box<dyn std::error::Error>> {
    let entity = TestEntity::new(ROOT, "Root", "synthetic/root.xml");
    let entity = if vehicle {
        entity.vehicle(VEHICLE, "")
    } else {
        entity.loadout(TestLoadout::Xml(XML.to_owned()))
    };
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(entity.record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("source missing")?;
    let tree = record.tree()?;
    let capture = Arc::new(Capture::new(&super::source(&record, &tree)?, None));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), capture.clone())
            .with_files(&files);
    let row = crate::loadout::read_with(
        &mut resolver,
        &mut PortReader::new(),
        &forge,
        Wanted {
            record_index: record.index(),
            guid: CatalogGuid(record.guid()),
            kind: EntityKind::Ship,
        },
        &capture,
    )
    .map_err(|failure| failure.reason().to_owned())?;
    assert!(
        row.is_some(),
        "the partial source representation remains retained"
    );
    Ok(capture.finish()?)
}

#[test]
fn missing_configured_files_retain_typed_target_paths_as_information() -> TestResult {
    for vehicle in [false, true] {
        let ledger = read(Reply::Missing, vehicle)?;
        let code = if vehicle {
            "vehicle_implementation"
        } else {
            "xml_loadout"
        };
        let diagnostic = ledger
            .field_diagnostics
            .iter()
            .find(|diagnostic| diagnostic.warning_code.as_deref() == Some(code))
            .ok_or("configured file diagnostic missing")?;
        assert_eq!(
            diagnostic.code,
            ExportDiagnosticCode::DanglingReference,
            "actual NotFound stays a represented missing target"
        );
        assert_eq!(
            diagnostic.severity,
            ExportDiagnosticSeverity::Information,
            "source absence does not invent a read failure"
        );
        assert_eq!(
            diagnostic
                .examples
                .first()
                .and_then(|example| example.field.as_deref()),
            Some(if vehicle { VEHICLE } else { XML }),
            "the exact configured target path is preserved"
        );
        assert!(
            ledger.derived_failures.is_empty(),
            "a represented missing file does not imply a failed source projection"
        );
    }
    Ok(())
}

#[test]
fn present_but_unreadable_configured_files_remain_blocking() -> TestResult {
    for vehicle in [false, true] {
        let ledger = read(Reply::Unreadable, vehicle)?;
        assert!(
            ledger
                .field_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity != ExportDiagnosticSeverity::Information),
            "a real producer read fault blocks normal publication"
        );
        assert!(
            !ledger
                .field_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == ExportDiagnosticCode::DanglingReference),
            "actual read errors never become fabricated NotFound"
        );
    }
    Ok(())
}

#[test]
fn malformed_present_configured_files_remain_blocking() -> TestResult {
    for vehicle in [false, true] {
        let ledger = read(Reply::Malformed, vehicle)?;
        assert!(
            ledger
                .field_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity != ExportDiagnosticSeverity::Information),
            "present malformed XML blocks normal publication"
        );
        assert!(
            !ledger
                .field_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == ExportDiagnosticCode::DanglingReference),
            "a parse failure never falls back to source absence"
        );
    }
    Ok(())
}
