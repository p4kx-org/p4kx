// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::sync::Arc;

use crate::dialog::Dialog;
use crate::output_target::{OutputScope, PickerFailure, TargetSelection, Witness};
use crate::{Language, OutputTargetText as T, Texts};

pub(super) struct Window {
    pub(super) texts: Texts,
    pub(super) target: TargetSelection,
    pub(super) parent_request: Option<Dialog>,
    pub(super) parent_requests: u32,
    pub(super) failure: Option<PickerFailure>,
    owner: Arc<()>,
}

impl Window {
    pub(super) fn new(language: Language, dialog: Dialog, invalid: bool) -> Self {
        let owner = Arc::new(());
        let scope = OutputScope {
            source: None,
            owner: Witness::of(&owner),
            subject: None,
            picture: None,
        };
        let mut target = TargetSelection::new();
        target.begin(
            dialog,
            scope,
            if invalid { "../escape" } else { "Prüfung Ä" }.to_owned(),
            None,
        );
        let ticket = target.pending().expect("selection opened").ticket();
        target.parent_reply(&ticket, Some(PathBuf::from(if cfg!(windows) {
            "C:\\Users\\Fixture\\Documents\\Output parent with a deliberately long folder name"
        } else {
            "/run/user/1000/doc/synthetic-directory/Output parent with a deliberately long folder name"
        })));
        Self {
            texts: Texts::new(language),
            target,
            parent_request: None,
            parent_requests: 0,
            failure: None,
            owner,
        }
    }

    pub(super) fn failed_picker(language: Language) -> Self {
        let mut window = Self::new(language, Dialog::ModelFile, true);
        window.target.cancel();
        window.failure = Some(PickerFailure::Spawn(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "synthetic chooser failure",
        )));
        window
    }

    pub(super) fn show(&mut self, ui: &egui::Ui) {
        if self.failure.is_some() {
            crate::ui::picker_failure(ui, &self.texts, &mut self.failure);
            return;
        }
        if let Some(dialog) = super::show(ui, &self.texts, &mut self.target) {
            self.parent_request = Some(dialog);
            self.parent_requests += 1;
        }
    }

    pub(super) fn scope(&self) -> OutputScope {
        OutputScope {
            source: None,
            owner: Witness::of(&self.owner),
            subject: None,
            picture: None,
        }
    }

    pub(super) fn name_label(&self) -> String {
        let dialog = self.target.pending().expect("selection open").dialog();
        self.texts
            .get(if dialog.is_package_output() {
                T::PackageName
            } else {
                T::FileName
            })
            .to_uppercase()
    }
}
