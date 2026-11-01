// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_character::Guid;

use crate::BodyVariant;

/// The rigs of the heads: the body variant, the CRC-32C of the rig's name, which is the
/// `gender_hash` of the DNA of the Characters made for it, the GUID of the body type the CHFs of
/// that rig hold, and the name.
pub(crate) const RIGS: [(BodyVariant, u32, Guid, &str); 2] = [
    (
        BodyVariant::Male,
        0xdd6c_67f6,
        Guid::from_stored([
            0x61, 0x4a, 0x6b, 0x14, 0xd5, 0x39, 0xf4, 0x25, 0x49, 0x8a, 0xb6, 0xdf, 0x86, 0xa4,
            0x99, 0xa9,
        ]),
        "protos_human_male_face_t1_pu",
    ),
    (
        BodyVariant::Female,
        0x9ef4_eb54,
        Guid::from_stored([
            0xad, 0x4c, 0xb0, 0xef, 0x94, 0x4a, 0x79, 0xd0, 0x53, 0xc2, 0xd3, 0xb4, 0x58, 0x25,
            0x38, 0xad,
        ]),
        "protos_human_female_face_t1_pu",
    ),
];

/// Returns what the file name `stem` adds to the name of a stock head rig, e.g. `eyes` for
/// `protos_human_male_face_t1_pu_eyes`, ignoring ASCII case; `None` for any other name.
pub(crate) fn rig_part(stem: &str) -> Option<&str> {
    RIGS.iter().find_map(|(_, _, _, rig)| {
        let rest = stem.get(rig.len()..)?;
        let head = stem.get(..rig.len())?;
        if !head.eq_ignore_ascii_case(rig) {
            return None;
        }
        rest.strip_prefix('_').filter(|part| !part.is_empty())
    })
}
