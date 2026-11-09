// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "quality_contract.rs"]
pub(crate) mod contract;
#[path = "quality_eligibility.rs"]
mod eligibility;
#[path = "quality_evidence.rs"]
pub(crate) mod evidence;
#[path = "quality_inverse.rs"]
mod inverse;
#[path = "quality_read.rs"]
pub(crate) mod read;
#[path = "quality_resource_links.rs"]
mod resource_links;
#[path = "quality_scan.rs"]
pub(crate) mod scan;

#[path = "quality_assembly.rs"]
mod assembly;
#[path = "quality_rows.rs"]
mod rows;

pub(crate) use assembly::{Assembly, Input, Retention, Step};
pub(crate) use rows::{QualityRows, QuantizationUse};
