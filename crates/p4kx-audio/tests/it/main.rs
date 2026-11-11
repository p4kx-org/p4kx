// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Integration tests of `p4kx-audio`, built as one test binary.

mod adpcm;
mod adpcm_blocks;
mod adpcm_signal;
mod bnk;
mod bnk_containers;
mod bnk_corrupt;
mod bnk_hirc;
mod bnk_layout;
mod bnk_music;
mod bnk_quirks;
mod bnk_sound_layout;
mod bnk_source;
mod bnk_source_action;
mod bnk_source_container;
mod bnk_source_event;
mod bnk_source_sound;
mod bnk_source_stid;
mod bnk_versions;
mod codec_tag_2;
mod loops;
mod loops_ogg;
mod media_class;
mod ogg;
mod opus;
mod opus_corrupt;
mod opus_layouts;
mod opus_signal;
mod pages;
mod rfc8251_vectors;
mod riff_padding;
mod vorbis;
mod vorbis_wav;
mod wav;
mod wem;
mod wem_annotations;
mod wem_chunks;
mod wem_source;
