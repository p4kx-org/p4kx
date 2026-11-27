// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! What the export dialog of an Entity refuses and what it passes on to the Blender package.

use crate::entity_package_window::{finished, manifest};
use crate::entity_picking::{TestResult, opened, select};
use egui_kittest::kittest::Queryable;
use p4kx_gui::{AssemblyExportForm, Language, PackageProblem};

#[test]
fn a_package_folder_inside_an_installation_is_refused_and_nothing_is_written() -> TestResult {
    let ((_dir, mut harness), channel) =
        crate::entity_picking::opened_in_installation(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    harness.state_mut().begin_assembly_export();
    let folder = channel.join("package");
    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?;
    setup.set_form(AssemblyExportForm::Package);
    setup.set_package_folder(folder.clone());

    assert!(matches!(
        setup.package_problem(),
        Some(PackageProblem::Protected(_))
    ));
    assert!(setup.refusal().is_some());
    assert!(!setup.is_ready());
    harness.run_steps(3);
    assert!(
        harness
            .query_by_label_contains("lies inside the Star Citizen installation")
            .is_some(),
        "the dialog says why"
    );
    assert!(!harness.state_mut().submit_assembly_export(|| {}));
    assert!(!folder.exists());
    Ok(())
}

#[test]
fn the_choices_of_the_dialog_reach_the_manifest() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish)?;
    select(&mut harness, "TEST_Ship")?;
    harness.state_mut().begin_assembly_export();
    let folder = dir.path().join("ship");
    let setup = harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?;
    setup.set_form(AssemblyExportForm::Package);
    setup.set_lod(2);
    setup.set_texture_mip(1);
    setup.set_vertex_colours(false);
    setup.set_package_folder(folder.clone());
    assert!(harness.state_mut().submit_assembly_export(|| {}));
    finished(&mut harness)?;

    let options = &manifest(&folder)?["options"];
    assert_eq!(options["lod"], 2);
    assert_eq!(options["texture_mip"], 1);
    assert_eq!(options["vertex_colours"], false);
    Ok(())
}
