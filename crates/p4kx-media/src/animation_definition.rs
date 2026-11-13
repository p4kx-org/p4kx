// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod links;
mod named_clip;
mod origins;
mod payload;
mod projection;
mod provider;
mod schema;
mod timeline;
mod timeline_payload;

pub(crate) use provider::AnimationDefinitionProvider;
pub(crate) use schema::bind_record_schema;
