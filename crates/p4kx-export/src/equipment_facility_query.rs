// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{record_query, rows};

pub use super::cargo_read::FacilityOwner;
pub use crate::catalog_v3::catalog_facility::FacilityKind;

record_query::unlocalized!(
    rows::Facilities,
    FacilityOwner,
    /// Selected original facility rows and their reached cargo-capacity evidence.
    FacilitySnapshot,
    with_facilities,
    with_facilities_from
);
