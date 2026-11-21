// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt;
use std::io;
use std::sync::mpsc;

use super::App;
use super::picker::{NativeDialog, NativeStartError};
use crate::dialog::Dialog;
use crate::output_target::PickerFailure;
use crate::{Language, Model};

#[derive(Debug)]
struct NativeCause;

impl fmt::Display for NativeCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("synthetic chooser failure")
    }
}

impl Error for NativeCause {}

fn app() -> App {
    App::new(Model::new(Language::BritishEnglish), None, None)
}

#[test]
fn a_native_spawn_failure_retains_its_original_cause_without_dispatch() {
    let mut app = app();
    app.accept_native_dialog_start(Err(NativeStartError {
        parent: None,
        source: io::Error::new(io::ErrorKind::PermissionDenied, NativeCause),
    }));
    let Some(PickerFailure::Spawn(error)) = app.dialog_failure.as_ref() else {
        panic!("a native chooser failure must remain visible");
    };
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(
        error
            .get_ref()
            .is_some_and(<dyn Error + Send + Sync>::is::<NativeCause>),
        "the original operating-system cause remains available"
    );
    assert!(app.dialog.is_none(), "no failed receiver is installed");
    assert!(app.take_dialog_result().is_none(), "no output dispatch");
}

#[test]
fn an_unexpected_disconnect_is_visible_and_user_cancellation_is_clean() {
    let mut disconnected = app();
    let (sender, receiver) = mpsc::channel();
    disconnected.dialog = Some(NativeDialog {
        dialog: Dialog::OpenArchive,
        receiver,
        parent: None,
    });
    drop(sender);
    assert!(disconnected.take_dialog_result().is_none());
    assert!(matches!(
        disconnected.dialog_failure,
        Some(PickerFailure::Disconnected)
    ));
    let mut cancelled = app();
    let (sender, receiver) = mpsc::channel();
    cancelled.dialog = Some(NativeDialog {
        dialog: Dialog::OpenArchive,
        receiver,
        parent: None,
    });
    sender.send(None).expect("the chooser receiver exists");
    assert!(cancelled.take_dialog_result().is_none());
    assert!(
        cancelled.dialog_failure.is_none(),
        "cancellation is not a failure"
    );
}
