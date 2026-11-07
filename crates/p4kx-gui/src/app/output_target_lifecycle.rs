// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;

use p4kx_extraction::RuntimeEnvironment;

use super::App;
use super::output_target_route;
use super::picker::{NativeDialog, NativeStartError};
use crate::dialog::Dialog;
use crate::output_target::{ParentTicket, PickerFailure};

impl App {
    /// Uses the checked startup environment only to choose the dialog presentation.
    #[must_use]
    pub const fn with_runtime_environment(mut self, runtime: RuntimeEnvironment) -> Self {
        self.runtime = runtime;
        self
    }

    pub(super) fn retain_output_target(&mut self) {
        let current = self
            .output_target
            .dialog()
            .and_then(|dialog| output_target_route::scope(&self.model, dialog));
        self.output_target.retain(current.as_ref());
    }

    fn prepare_parent_dialog(&mut self, dialog: Dialog) -> Option<ParentTicket> {
        let scope = output_target_route::scope(&self.model, dialog)?;
        if let Some(pending) = self.output_target.pending() {
            return (pending.dialog() == dialog && pending.scope() == &scope)
                .then(|| pending.ticket());
        }
        let name = output_target_route::name(&self.model, dialog)?;
        self.output_target.begin(dialog, scope, name, None);
        None
    }

    pub(super) fn uses_parent_dialog(&self, dialog: Dialog) -> bool {
        self.runtime == RuntimeEnvironment::Flatpak && dialog.is_parent_output()
    }

    pub(super) fn start_dialog(&mut self, ctx: &egui::Context, dialog: Dialog) {
        if self.dialog.is_some() || self.dialog_failure.is_some() {
            return;
        }
        let parent = if self.uses_parent_dialog(dialog) {
            let Some(ticket) = self.prepare_parent_dialog(dialog) else {
                ctx.request_repaint();
                return;
            };
            Some(ticket)
        } else {
            None
        };
        let native = NativeDialog::start(ctx, &self.model, dialog, parent);
        self.accept_native_dialog_start(native);
        ctx.request_repaint();
    }

    pub(super) fn accept_native_dialog_start(
        &mut self,
        native: Result<NativeDialog, NativeStartError>,
    ) {
        match native {
            Ok(native) => self.dialog = Some(native),
            Err(error) => {
                if let Some(ticket) = error.parent {
                    self.output_target
                        .picker_failed(&ticket, PickerFailure::Spawn(error.source));
                } else {
                    self.dialog_failure = Some(PickerFailure::Spawn(error.source));
                }
            }
        }
    }

    pub(super) fn take_dialog_result(&mut self) -> Option<(Dialog, PathBuf)> {
        let current = self
            .output_target
            .dialog()
            .and_then(|dialog| output_target_route::scope(&self.model, dialog));
        self.output_target.retain(current.as_ref());
        if let Some(choice) = self.output_target.take_confirmed(current.as_ref()) {
            return Some((choice.dialog, choice.path));
        }
        let native = self.dialog.as_ref()?;
        let picked = match native.receiver.try_recv() {
            Ok(path) => path,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                if let Some(native) = self.dialog.take() {
                    if let Some(ticket) = native.parent {
                        self.output_target
                            .picker_failed(&ticket, PickerFailure::Disconnected);
                    } else {
                        self.dialog_failure = Some(PickerFailure::Disconnected);
                    }
                }
                return None;
            }
        };
        let native = self.dialog.take()?;
        if let Some(ticket) = native.parent {
            self.output_target.parent_reply(&ticket, picked);
            return None;
        }
        picked.map(|path| (native.dialog, path))
    }
}
