// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::sound::wwise_path;
#[path = "cig_audio_field.rs"]
pub(crate) mod cig_audio_field;
#[path = "cig_audio_role.rs"]
pub(crate) mod cig_audio_role;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AudioControlSource {
    Atl,
    CigAudio,
}

impl AudioControlSource {
    pub(crate) fn from_path(path: &str) -> Option<Self> {
        if wwise_path::is_atl_definition(path) {
            return Some(Self::Atl);
        }
        let mut components = path.split(['/', '\\']);
        let matches = ["Data", "Libs", "CIGAudio", "CIGAudioData.bin"]
            .into_iter()
            .all(|component| {
                components
                    .next()
                    .is_some_and(|actual| actual.eq_ignore_ascii_case(component))
            });
        (matches && components.next().is_none()).then_some(Self::CigAudio)
    }
}
