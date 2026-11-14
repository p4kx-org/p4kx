// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Geometry fixture selection and evidence without audio-only canary APIs.

#[path = "case.rs"]
mod case;
#[path = "family.rs"]
mod family;
#[path = "geometry.rs"]
mod geometry;
#[path = "index.rs"]
mod index;
#[path = "observer.rs"]
mod observer;
#[path = "geometry_probe.rs"]
mod probe;
#[path = "report.rs"]
mod report;
#[path = "source.rs"]
mod source;
#[path = "source_checks.rs"]
mod source_checks;

pub(crate) use case::{Case, DockedVehicles, Requirements};
pub(crate) use family::Family;
pub(crate) use geometry::Prepared;
pub(crate) use index::{Identity, Index};
pub(crate) use report::{absent, finish, run, selected};
pub(crate) use source::with_source;
pub(super) use source_checks::SourceCheck;

pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn is_invariant(error: &(dyn std::error::Error + 'static)) -> bool {
    error.is::<SourceCheck>()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
#[cfg(test)]
#[path = "xml_fault_tests.rs"]
mod xml_fault_tests;
