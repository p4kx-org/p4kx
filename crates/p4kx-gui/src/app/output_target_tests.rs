// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::sync::{Arc, mpsc};

use p4kx_extraction::RuntimeEnvironment;

use super::App;
use super::picker::NativeDialog;
use crate::dialog::Dialog;
use crate::output_target::test_routes::OUTPUT_DIALOGS;
use crate::output_target::{OutputScope, Witness};
use crate::{Language, Model};

fn app(runtime: RuntimeEnvironment) -> App {
    App::new(Model::new(Language::BritishEnglish), None, None).with_runtime_environment(runtime)
}

#[test]
fn only_the_checked_flatpak_presentation_uses_parent_and_name() {
    let native = app(RuntimeEnvironment::Native);
    let flatpak = app(RuntimeEnvironment::Flatpak);
    for dialog in OUTPUT_DIALOGS {
        assert!(!native.uses_parent_dialog(dialog), "{dialog:?}");
        assert!(flatpak.uses_parent_dialog(dialog), "{dialog:?}");
    }
    for dialog in [
        Dialog::ExtractionFolder,
        Dialog::ConversionOutput,
        Dialog::RecordExportFolder,
        Dialog::SoundsFolder,
        Dialog::OpenArchive,
    ] {
        assert!(!flatpak.uses_parent_dialog(dialog), "{dialog:?}");
    }
}

#[test]
fn native_output_replies_keep_every_existing_route_and_path() {
    let mut app = app(RuntimeEnvironment::Native);
    let path = PathBuf::from("original chooser path");
    for dialog in OUTPUT_DIALOGS {
        let (sender, receiver) = mpsc::channel();
        app.dialog = Some(NativeDialog {
            dialog,
            receiver,
            parent: None,
        });
        sender.send(Some(path.clone())).expect("receiver exists");
        assert_eq!(app.take_dialog_result(), Some((dialog, path.clone())));
        assert!(app.take_dialog_result().is_none(), "single native delivery");
    }
}

#[test]
fn a_cancelled_parent_reply_never_becomes_an_output_dispatch() {
    let mut app = app(RuntimeEnvironment::Flatpak);
    let owner = Arc::new(());
    let scope = OutputScope {
        source: None,
        owner: Witness::of(&owner),
        subject: None,
        picture: None,
    };
    app.output_target.begin(
        Dialog::DiagnosticCatalogFile,
        scope,
        "test.json".to_owned(),
        None,
    );
    let ticket = app
        .output_target
        .pending()
        .expect("selection exists")
        .ticket();
    let (sender, receiver) = mpsc::channel();
    app.dialog = Some(NativeDialog {
        dialog: Dialog::DiagnosticCatalogFile,
        receiver,
        parent: Some(ticket),
    });
    app.output_target.cancel();
    sender
        .send(Some(PathBuf::from("late parent")))
        .expect("receiver exists");
    assert!(app.take_dialog_result().is_none());
    assert!(app.output_target.pending().is_none());
    assert!(app.dialog.is_none(), "late native slot drained");
}

#[test]
fn no_current_export_owner_means_no_pending_or_confirmed_dispatch() {
    let mut app = app(RuntimeEnvironment::Flatpak);
    let owner = Arc::new(());
    let scope = OutputScope {
        source: None,
        owner: Witness::of(&owner),
        subject: None,
        picture: None,
    };
    app.output_target
        .begin(Dialog::ModelFile, scope, "model.glb".to_owned(), None);
    app.retain_output_target();
    assert!(app.output_target.pending().is_none(), "no open model setup");
    assert!(app.take_dialog_result().is_none());
}
