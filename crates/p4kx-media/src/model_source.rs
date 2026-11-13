// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "model_source/origins.rs"]
mod origins;
#[path = "model_source/payload.rs"]
mod payload;
#[path = "model_source/provider.rs"]
mod provider;
#[path = "model_source/schema.rs"]
mod schema;

pub(crate) use provider::ModelSourceProvider;
pub(crate) use schema::bind_record_schema;
