// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Development-only field accounting and published offline reference comparisons.

mod accounting;
mod audit_scope;
mod baseline;
#[path = "../real_archive/catalog_v3_consumer.rs"]
mod catalog_v3_consumer;
mod compare;
mod coverage;
mod coverage_v3_tests;
mod error;
mod fixtures;
mod inventory;
mod joins;
mod mission_property_metadata;
mod mission_property_metadata_fixture;
mod mutations_accounting;
mod mutations_audit_scope;
mod mutations_boundaries;
mod mutations_compare;
mod mutations_mission_property_metadata;
mod mutations_published;
mod mutations_resource_metadata;
mod mutations_shape;
mod path;
mod published;
mod published_io;
mod published_model;
mod published_validate;
mod real_archive;
mod resource_declarations;
mod resource_metadata;
mod resource_metadata_fixture;
mod resource_metadata_output;
mod schema;
mod source;
mod source_fixture;
mod source_scope;
#[path = "../it/spawns_fixture/mod.rs"]
mod spawns_fixture;
mod tagged_fixture;
mod units;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[expect(
    clippy::print_stdout,
    reason = "aggregate verification counts are the test result"
)]
fn report(message: &str) {
    println!("{message}");
}
