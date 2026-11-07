// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::sync::Arc;

use super::test_routes::OUTPUT_DIALOGS;
use super::{NameProblem, OutputScope, OwnerLease, PickerFailure, TargetSelection, Witness};
use crate::dialog::Dialog;
use crate::{ComparisonSide, QueryExportFormat};

fn parent() -> PathBuf {
    PathBuf::from(if cfg!(windows) {
        "C:\\chosen-parent"
    } else {
        "/chosen-parent"
    })
}

fn scope(owner: &Arc<()>) -> OutputScope {
    OutputScope {
        source: Some(Witness::of(owner)),
        owner: Witness::of(owner),
        subject: None,
        picture: None,
    }
}

fn opened(dialog: Dialog, name: &str) -> (Arc<()>, TargetSelection) {
    let owner = Arc::new(());
    let mut state = TargetSelection::new();
    state.begin(dialog, scope(&owner), name.to_owned(), None);
    let ticket = state.pending().expect("selection exists").ticket();
    state.parent_reply(&ticket, Some(parent()));
    (owner, state)
}

#[test]
fn a_selected_parent_and_one_component_resolve_the_exact_path() {
    let (_owner, state) = opened(Dialog::AssemblyPackage, "Paket Ä");
    assert_eq!(
        state.pending().expect("open").path(),
        Some(parent().join("Paket Ä"))
    );
}

#[test]
fn no_parent_is_inferred_from_a_proposed_name() {
    let owner = Arc::new(());
    let mut state = TargetSelection::new();
    state.begin(
        Dialog::CatalogFile,
        scope(&owner),
        "catalog.json".to_owned(),
        None,
    );
    assert!(
        !state.confirm(),
        "no Directory chooser reply means no destination"
    );
    assert!(state.pending().expect("open").parent().is_none());
}

#[test]
fn component_validation_rejects_escape_and_normalization() {
    for name in [
        "",
        ".",
        "..",
        "one/two",
        "/absolute",
        "name/",
        "one/../two",
        "one//two",
    ] {
        let (_owner, state) = opened(Dialog::AssemblyPackage, name);
        assert!(
            state.pending().expect("open").name_problem().is_some(),
            "{name:?}"
        );
        assert!(state.pending().expect("open").path().is_none(), "{name:?}");
    }
}

#[test]
fn control_and_nul_names_have_a_specific_problem() {
    for name in ["bad\0name", "bad\nname", "bad\tname"] {
        let (_owner, state) = opened(Dialog::ModelFile, name);
        assert_eq!(
            state.pending().expect("open").name_problem(),
            Some(NameProblem::Control)
        );
    }
}

#[cfg(windows)]
#[test]
fn windows_prefixes_and_separators_cannot_escape_the_parent() {
    for name in [
        "C:relative",
        "C:\\absolute",
        "\\\\server\\share",
        "one\\two",
        "name\\",
    ] {
        let (_owner, state) = opened(Dialog::AssemblyPackage, name);
        assert_eq!(
            state.pending().expect("open").name_problem(),
            Some(NameProblem::Component)
        );
    }
}

#[cfg(unix)]
#[test]
fn a_literal_backslash_is_preserved_on_unix() {
    let (_owner, state) = opened(Dialog::AssemblyPackage, "part\\name");
    assert_eq!(
        state.pending().expect("open").path(),
        Some(parent().join("part\\name"))
    );
}

#[test]
fn existing_glb_and_catalog_normalization_is_displayed_before_confirmation() {
    for (dialog, name, expected) in [
        (Dialog::ModelFile, "model", "model.glb"),
        (Dialog::CharacterFile, "character.other", "character.other"),
        (Dialog::AssemblyPackage, "package", "package"),
        (
            Dialog::CatalogFile,
            "catalog.json.json.zst",
            "catalog.json.zst",
        ),
    ] {
        let (_owner, state) = opened(dialog, name);
        assert_eq!(
            state.pending().expect("open").path(),
            Some(parent().join(expected))
        );
    }
}

#[test]
fn cancelling_the_parent_picker_preserves_the_selected_parent_and_name() {
    let (_owner, mut state) = opened(Dialog::ModelFile, "retained.glb");
    let ticket = state.pending().expect("open").ticket();
    state.parent_reply(&ticket, None);
    assert_eq!(
        state.pending().expect("open").path(),
        Some(parent().join("retained.glb"))
    );
}

#[test]
fn a_relative_picker_reply_preserves_the_previous_parent_with_an_explicit_failure() {
    let (_owner, mut state) = opened(Dialog::ModelFile, "retained.glb");
    let ticket = state.pending().expect("open").ticket();
    state.parent_reply(&ticket, Some(PathBuf::from("relative-parent")));
    let selected = state.pending().expect("open");
    assert_eq!(selected.parent(), Some(parent().as_path()));
    assert!(matches!(
        selected.picker_failure(),
        Some(PickerFailure::RelativePath)
    ));
}

#[test]
fn cancelled_and_reopened_selections_ignore_the_old_native_reply() {
    let (owner, mut state) = opened(Dialog::AssemblyPackage, "old");
    let old = state.pending().expect("open").ticket();
    state.cancel();
    state.begin(
        Dialog::AssemblyPackage,
        scope(&owner),
        "new".to_owned(),
        None,
    );
    state.parent_reply(&old, Some(parent()));
    assert!(state.pending().expect("new selection").parent().is_none());
}

#[test]
fn a_new_source_or_setup_invalidates_both_pending_and_confirmed_choices() {
    let (owner, mut state) = opened(Dialog::ModelFile, "test.glb");
    let other = Arc::new(());
    state.retain(Some(&scope(&other)));
    assert!(
        state.pending().is_none(),
        "source replacement closes selection"
    );
    let (_current_owner, mut state) = opened(Dialog::ModelFile, "test.glb");
    assert!(state.confirm());
    assert!(
        state.take_confirmed(Some(&scope(&owner))).is_none(),
        "old setup cannot receive it"
    );
}

#[test]
fn setup_leases_distinguish_recreation_and_preserve_an_ordinary_clone() {
    let first = OwnerLease::default();
    let cloned = first.clone();
    assert_eq!(first.witness(), cloned.witness(), "same setup snapshot");
    assert_ne!(
        first.witness(),
        OwnerLease::default().witness(),
        "reopened setup"
    );
}

#[test]
fn cancelled_selection_clears_focus_and_pending_dispatch() {
    let (owner, mut state) = opened(Dialog::ModelFile, "test.glb");
    let selected = state.pending_mut().expect("selection exists");
    assert!(selected.take_focus(), "focus requested once");
    assert!(!selected.take_focus(), "no focus stealing on later frames");
    state.cancel();
    assert!(state.pending().is_none());
    assert!(state.take_confirmed(Some(&scope(&owner))).is_none());
}

#[test]
fn picker_failures_keep_the_actual_filesystem_error_and_previous_destination() {
    let (_owner, mut state) = opened(Dialog::ModelFile, "test.glb");
    let ticket = state.pending().expect("selection exists").ticket();
    state.picker_failed(
        &ticket,
        PickerFailure::Spawn(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "synthetic spawn refusal",
        )),
    );
    let selected = state.pending().expect("selection remains");
    assert_eq!(selected.path(), Some(parent().join("test.glb")));
    let Some(PickerFailure::Spawn(error)) = selected.picker_failure() else {
        panic!("typed native failure retained");
    };
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(error.to_string(), "synthetic spawn refusal");
}

#[test]
fn changing_an_existing_result_generation_invalidates_the_choice() {
    let (owner, mut state) = opened(Dialog::QueryExport(QueryExportFormat::Csv), "query.csv");
    let mut changed = scope(&owner);
    changed.owner = changed.owner.at(1, 0);
    state.retain(Some(&changed));
    assert!(
        state.pending().is_none(),
        "same DataForge, different query result"
    );
}

#[test]
fn confirm_dispatches_each_existing_output_route_exactly_once() {
    for dialog in OUTPUT_DIALOGS {
        let (owner, mut state) = opened(dialog, "output");
        let expected = state.pending().expect("open").path();
        assert!(state.confirm(), "{dialog:?}");
        assert!(!state.confirm(), "second confirm has no pending selection");
        let choice = state
            .take_confirmed(Some(&scope(&owner)))
            .expect("one dispatch");
        assert_eq!(choice.dialog, dialog);
        assert_eq!(Some(choice.path), expected);
        assert!(
            state.take_confirmed(Some(&scope(&owner))).is_none(),
            "{dialog:?}"
        );
    }
}

#[test]
fn cancelling_a_diagnostic_selection_does_not_authorize_the_next_catalog() {
    let (owner, mut state) = opened(Dialog::DiagnosticCatalogFile, "diagnostic.json.zst");
    let late = state.pending().expect("open").ticket();
    state.cancel();
    assert!(state.take_confirmed(Some(&scope(&owner))).is_none());
    state.begin(
        Dialog::CatalogFile,
        scope(&owner),
        "catalog.json.zst".to_owned(),
        None,
    );
    state.parent_reply(&late, Some(parent()));
    assert!(
        !state.confirm(),
        "the cancelled diagnostic parent reply grants nothing"
    );
    let current = state.pending().expect("normal catalog selection").ticket();
    state.parent_reply(&current, Some(parent()));
    assert!(state.confirm());
    assert_eq!(
        state
            .take_confirmed(Some(&scope(&owner)))
            .expect("normal dispatch")
            .dialog,
        Dialog::CatalogFile
    );
}

#[test]
fn folder_outputs_and_inputs_keep_the_native_dialog_route() {
    for dialog in [
        Dialog::OpenArchive,
        Dialog::OpenDataForge,
        Dialog::AddFolder,
        Dialog::AddArchive,
        Dialog::ExtractionFolder,
        Dialog::ConversionInput,
        Dialog::ConversionOutput,
        Dialog::RecordExportFolder,
        Dialog::SoundsFolder,
        Dialog::CharacterChf,
        Dialog::ComparisonArchive(ComparisonSide::Old),
    ] {
        assert!(!dialog.is_parent_output(), "{dialog:?}");
    }
    for dialog in OUTPUT_DIALOGS {
        assert!(dialog.is_parent_output(), "{dialog:?}");
    }
}
