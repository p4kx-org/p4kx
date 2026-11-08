// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Synthetic media contract, source preservation and provider integration tests.

mod archive;
mod archive_layout;
mod budgets;
mod capabilities;
mod entry_metadata_provider;
mod entry_metadata_schema;
mod entry_metadata_words;
mod interruptions;
#[cfg(windows)]
mod junctions;
mod layout_artifacts;
mod layout_fixture;
mod layout_health;
mod layout_legacy_index;
mod layout_mutations;
mod layout_schema;
mod layout_scope;
mod layout_support;
mod material_lookup;
mod material_lookup_cycles;
mod material_lookup_failures;
mod material_lookup_limits;
mod material_lookup_support;
mod material_provider_failures;
mod material_provider_schema;
mod material_provider_sources;
mod material_provider_status;
mod material_support;
mod non_finite;
mod owned_outputs;
mod process_lock;
mod providers;
mod recovery;
mod reference_context;
mod references;
mod required_cache_integrity;
mod required_input_failures;
mod required_input_support;
mod required_inputs;
mod required_selection;
mod retained_entry_names;
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
