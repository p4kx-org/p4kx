// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod picker_failure;
mod selection;
mod witness;

pub(crate) use picker_failure::PickerFailure;
pub(crate) use selection::{NameProblem, ParentTicket, Selection, TargetSelection};
pub(crate) use witness::{OwnerLease, Witness};

use crate::ui_tab::OutputPicture;

/// The source and exact owner of a pending output choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutputScope {
    pub(crate) source: Option<Witness>,
    pub(crate) owner: Witness,
    pub(crate) subject: Option<Witness>,
    pub(crate) picture: Option<OutputPicture>,
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod test_routes;
