// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The opened Catalog input and its checked original Record/read frames.

#[path = "original_catalog_input.rs"]
mod input;
#[path = "original_record_scope.rs"]
mod scope;

pub(crate) use input::OriginalCatalogInput;
pub(crate) use scope::OriginalRecordScope;

#[cfg(test)]
#[path = "original_capture_test_support.rs"]
pub(crate) mod test_support;

#[cfg(test)]
#[path = "original_catalog_input_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "ship_pool_test_input.rs"]
pub(crate) mod ship_pool_test_input;

#[cfg(test)]
#[path = "original_entry_success_binding_tests.rs"]
mod entry_success_binding_tests;
