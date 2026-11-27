// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic consumer cache boundary using public selective queries.

#[path = "catalog_consumer_refresh_cache.rs"]
mod cache;
#[path = "catalog_consumer_refresh_fixture.rs"]
mod fixture;
#[path = "catalog_consumer_refresh_queries.rs"]
mod queries;
#[path = "catalog_consumer_refresh_tests.rs"]
mod tests;

#[path = "catalog_consumer_refresh_mining.rs"]
mod mining;

#[path = "catalog_consumer_refresh_origins.rs"]
mod origins;
