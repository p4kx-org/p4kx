// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Weak};

use super::OutputScope;
use super::PickerFailure;
use crate::dialog::Dialog;

/// Why an output name cannot be joined to its selected parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NameProblem {
    Empty,
    Component,
    Control,
}

/// A native parent reply belongs to exactly one selection lifetime.
#[derive(Debug)]
pub(crate) struct ParentTicket(Weak<()>);

/// An owned output route and its displayed, resolved path.
#[derive(Debug)]
pub(crate) struct OutputChoice {
    pub(crate) dialog: Dialog,
    pub(crate) path: PathBuf,
    scope: OutputScope,
}

/// The fields of one uncommitted parent-and-name choice.
#[derive(Debug)]
pub(crate) struct Selection {
    dialog: Dialog,
    scope: OutputScope,
    lifetime: Arc<()>,
    parent: Option<PathBuf>,
    name: String,
    focus_name: bool,
    picker_failure: Option<PickerFailure>,
}

impl Selection {
    pub(crate) const fn dialog(&self) -> Dialog {
        self.dialog
    }

    pub(crate) const fn scope(&self) -> &OutputScope {
        &self.scope
    }

    pub(crate) fn parent(&self) -> Option<&Path> {
        self.parent.as_deref()
    }

    pub(crate) const fn name_mut(&mut self) -> &mut String {
        &mut self.name
    }

    pub(crate) fn take_focus(&mut self) -> bool {
        std::mem::take(&mut self.focus_name)
    }

    pub(crate) const fn picker_failure(&self) -> Option<&PickerFailure> {
        self.picker_failure.as_ref()
    }

    pub(crate) fn ticket(&self) -> ParentTicket {
        ParentTicket(Arc::downgrade(&self.lifetime))
    }

    pub(crate) fn name_problem(&self) -> Option<NameProblem> {
        if self.name.is_empty() {
            return Some(NameProblem::Empty);
        }
        if self.name.chars().any(char::is_control) {
            return Some(NameProblem::Control);
        }
        let mut components = Path::new(&self.name).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(name)), None) if name == std::ffi::OsStr::new(&self.name) => {
                None
            }
            _ => Some(NameProblem::Component),
        }
    }

    pub(crate) fn path(&self) -> Option<PathBuf> {
        if self.name_problem().is_some() {
            return None;
        }
        let path = self.parent.as_ref()?.join(&self.name);
        Some(
            if self.dialog.is_glb_output() && path.extension().is_none() {
                path.with_extension("glb")
            } else if matches!(
                self.dialog,
                Dialog::CatalogFile | Dialog::DiagnosticCatalogFile
            ) {
                crate::catalog_export::normalised_catalog_file(path)
            } else {
                path
            },
        )
    }
}

/// The modal and a single consumable output dispatch.
#[derive(Debug, Default)]
pub(crate) struct TargetSelection {
    pending: Option<Selection>,
    confirmed: Option<OutputChoice>,
}

impl TargetSelection {
    pub(crate) const fn new() -> Self {
        Self {
            pending: None,
            confirmed: None,
        }
    }

    pub(crate) const fn pending(&self) -> Option<&Selection> {
        self.pending.as_ref()
    }

    pub(crate) const fn pending_mut(&mut self) -> Option<&mut Selection> {
        self.pending.as_mut()
    }

    pub(crate) fn begin(
        &mut self,
        dialog: Dialog,
        scope: OutputScope,
        name: String,
        parent: Option<PathBuf>,
    ) {
        self.confirmed = None;
        self.pending = Some(Selection {
            dialog,
            scope,
            lifetime: Arc::new(()),
            parent,
            name,
            focus_name: true,
            picker_failure: None,
        });
    }

    pub(crate) fn cancel(&mut self) {
        self.pending = None;
        self.confirmed = None;
    }

    pub(crate) fn retain(&mut self, scope: Option<&OutputScope>) {
        let expected = self
            .pending
            .as_ref()
            .map(|pending| &pending.scope)
            .or_else(|| self.confirmed.as_ref().map(|choice| &choice.scope));
        if expected.is_some() && expected != scope {
            self.cancel();
        }
    }

    pub(crate) fn dialog(&self) -> Option<Dialog> {
        self.pending
            .as_ref()
            .map(|pending| pending.dialog)
            .or_else(|| self.confirmed.as_ref().map(|choice| choice.dialog))
    }

    pub(crate) fn parent_reply(&mut self, ticket: &ParentTicket, path: Option<PathBuf>) {
        if let Some(pending) = self.pending.as_mut()
            && ticket.0.ptr_eq(&Arc::downgrade(&pending.lifetime))
        {
            pending.picker_failure = None;
            if let Some(path) = path {
                if path.is_absolute() {
                    pending.parent = Some(path);
                } else {
                    pending.picker_failure = Some(PickerFailure::RelativePath);
                }
            }
        }
    }

    pub(crate) fn picker_failed(&mut self, ticket: &ParentTicket, failure: PickerFailure) {
        if let Some(pending) = self.pending.as_mut()
            && ticket.0.ptr_eq(&Arc::downgrade(&pending.lifetime))
        {
            pending.picker_failure = Some(failure);
        }
    }

    pub(crate) fn confirm(&mut self) -> bool {
        let Some(path) = self.pending.as_ref().and_then(Selection::path) else {
            return false;
        };
        let Some(selection) = self.pending.take() else {
            return false;
        };
        self.confirmed = Some(OutputChoice {
            dialog: selection.dialog,
            path,
            scope: selection.scope,
        });
        true
    }

    pub(crate) fn take_confirmed(&mut self, scope: Option<&OutputScope>) -> Option<OutputChoice> {
        self.confirmed
            .take()
            .filter(|choice| Some(&choice.scope) == scope)
    }
}
