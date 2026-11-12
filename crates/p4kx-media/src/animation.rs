// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod controller_projection;
mod origin;
mod payload;
mod projection;
mod provider;
mod schema;

pub(crate) use provider::AnimationProvider;
pub(crate) use schema::bind_record_schema;
