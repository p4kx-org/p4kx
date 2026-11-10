// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx-mcp` end to end: the server runs as a child process and is spoken to over stdio, with a
//! synthetic Library folder and Archive.

mod archive_key;
mod archive_key_cache;
mod archive_key_note;
mod archives;
mod audio;
mod audio_candidate_assertions;
mod audio_candidate_fixture;
mod audio_complete_candidates;
mod audio_followups;
mod audio_limits;
mod audio_objects;
mod audio_wem;
mod channel_notice;
mod complete_audio_events;
mod complete_loadout_notes;
mod complete_loadout_notes_fixture;
mod complete_loadout_notes_source;
mod containers;
mod dataforge_cache;
mod dataforge_file;
mod definitions;
mod entity_loadout;
mod entity_loadout_notes;
mod entity_loadout_pick;
mod entries;
mod exact_audio_assertions;
mod exact_audio_fixture;
mod exact_audio_paging;
mod exact_audio_source;
mod exact_audio_strings;
mod exact_audio_workers;
mod exact_loadout_assertions;
mod exact_loadout_fixture;
mod exact_loadout_source;
mod exact_loadout_strings;
mod exact_loadout_values;
mod exact_material_assertions;
mod exact_material_fixture;
mod exact_material_late_item;
mod exact_material_strings;
mod forge;
mod image_preview;
mod legal_notice;
mod model_animations;
mod models;
mod mtl_summary;
mod query;
mod query_selection;
mod read;
mod read_only;
mod read_text;
mod real_archive;
mod real_archive_audio;
mod real_archive_model;
mod real_archive_query;
mod records;
mod reopen;
mod support;
mod tool_list;
