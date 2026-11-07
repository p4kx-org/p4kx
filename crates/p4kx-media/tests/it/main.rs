// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Synthetic media contract, source preservation and provider integration tests.

mod archive;
mod budgets;
mod capabilities;
mod entry_metadata_schema;
mod interruptions;
#[cfg(windows)]
mod junctions;
mod non_finite;
mod owned_outputs;
mod process_lock;
mod providers;
mod recovery;
mod reference_context;
mod references;
mod schema;
mod shader_provider_failures;
mod shader_provider_schema;
mod shader_provider_sources;
mod shader_provider_status;
mod shader_support;
mod source_errors;
mod streaming;
mod support;
mod transactions;
mod unsupported_source;
mod write_protection;
