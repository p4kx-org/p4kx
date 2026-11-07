// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Arc;

use egui_kittest::kittest::Queryable;
use p4kx_extraction::WriteProtectionError;
use p4kx_gui::{AssemblyExport, AssemblyExportStatus, Language, PackageProblem, ProtectionText};

use crate::entity_picking::{TestResult, opened, select};

use super::{finished, start};

#[test]
fn a_folder_that_cannot_be_created_fails_with_the_reason() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"a file")?;

    let target = blocker.join("ship");
    if start(&mut harness, &target)? {
        finished(&mut harness)?;
        assert!(matches!(
            harness
                .state()
                .assembly_export()
                .map(AssemblyExport::status),
            Some(AssemblyExportStatus::Failed(_))
        ));
        assert!(harness.query_by_label("Export failed").is_some());
    } else {
        let setup = harness.state().assembly_export_setup().ok_or("no setup")?;
        assert!(!setup.is_ready());
        let Some(PackageProblem::Protected(WriteProtectionError::TargetIdentity {
            target: refused,
            cause,
        })) = setup.package_problem()
        else {
            return Err("the invalid parent did not retain its identity refusal".into());
        };
        assert_eq!(refused, &target);
        assert!(
            cause
                .source()
                .and_then(|source| source.downcast_ref::<Arc<dyn Error + Send + Sync>>())
                .and_then(|source| source.as_ref().downcast_ref::<std::io::Error>())
                .is_some_and(|source| source.kind() == std::io::ErrorKind::NotADirectory)
        );
        harness.run_steps(3);
        let notice = harness.state().texts().get(ProtectionText::TargetIdentity);
        assert!(harness.query_by_label(&notice).is_some());
    }
    assert_eq!(std::fs::read(&blocker)?, b"a file");
    assert!(!target.exists());
    Ok(())
}
