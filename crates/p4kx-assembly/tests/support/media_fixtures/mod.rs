// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Capability-bound media fixtures with exact physical source identities.

mod case;
mod checks;
mod family;
mod family_audio;
mod geometry;
mod index;
mod observer;
mod probe;
mod report;
mod source;
mod source_checks;

pub(crate) use case::{Case, DockedVehicles, Requirements};
pub(crate) use checks::Check;
pub(crate) use family::Family;
pub(crate) use index::{Identity, Index};
pub(crate) use probe::Prepared;
pub(crate) use report::{absent, finish, run, selected};
pub(crate) use source::with_source;
pub(crate) use source_checks::{SourceCheck, require};

pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn is_invariant(error: &(dyn std::error::Error + 'static)) -> bool {
    error.is::<Check>() || error.is::<SourceCheck>()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod xml_fault_tests;
