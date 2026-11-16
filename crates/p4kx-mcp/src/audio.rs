// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "audio_entity.rs"]
mod entity;
#[path = "audio_event.rs"]
pub(super) mod event;
#[path = "audio_event_origins.rs"]
mod event_origins;
#[path = "audio_header.rs"]
mod header;
#[path = "audio_location.rs"]
pub(super) mod location;
#[path = "audio_trigger_occurrence_origins.rs"]
mod occurrence_origins;
#[path = "audio_resolve.rs"]
mod resolve;
#[path = "audio_search.rs"]
mod search;
#[path = "audio_surveys.rs"]
mod surveys;
#[path = "audio_users.rs"]
mod users;

pub(crate) use entity::entity_sounds;
pub(crate) use resolve::resolve;
pub(crate) use search::search;

#[cfg(test)]
#[path = "audio_envelope_tests.rs"]
mod tests;
