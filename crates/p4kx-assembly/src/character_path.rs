// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Which model paths hold a character and which a prop of the world.

use crate::path_match::starts_with_path;

/// The folder of the human body rigs, with a trailing separator.
const HUMAN_RIGS: &str = "Objects/Characters/Human/";

/// The folder of the character models, with a trailing separator.
const CHARACTERS: &str = "Objects/Characters/";

/// The folder an Archive path starts with that a Record's path leaves out.
const DATA_FOLDER: &str = "Data/";

/// The extensions of the files that hold a character: definition, skeleton and skin.
const CHARACTER_EXTENSIONS: [&str; 3] = ["cdf", "chr", "skin"];

/// Returns `path` without a leading separator and without the `Data/` folder.
pub(crate) fn below_data(path: &str) -> &str {
    let relative = path.trim_start_matches(['/', '\\']);
    if starts_with_path(relative, DATA_FOLDER) {
        relative.get(DATA_FOLDER.len()..).unwrap_or_default()
    } else {
        relative
    }
}

/// Returns whether `relative` ends in the extension of a character file.
fn has_character_extension(relative: &str) -> bool {
    relative.rsplit_once('.').is_some_and(|(_, extension)| {
        CHARACTER_EXTENSIONS
            .iter()
            .any(|wanted| wanted.eq_ignore_ascii_case(extension))
    })
}

/// Returns whether `path` names a `.cdf`, `.chr` or `.skin` file outside the human body rigs.
pub(crate) fn is_rigged_path_outside_human_rigs(path: &str) -> bool {
    let relative = below_data(path);
    has_character_extension(relative) && !starts_with_path(relative, HUMAN_RIGS)
}

/// Returns whether `path` lies in the characters folder, whatever the file.
fn is_in_characters_folder(path: &str) -> bool {
    starts_with_path(below_data(path), CHARACTERS)
}

/// Returns whether `path` names a prop of the world: a model that is neither in the characters
/// folder nor a `.cdf`, `.chr` or `.skin` file, such as the cardboard box of an item that is not
/// worn on the body.
pub(crate) fn is_world_prop_path(path: &str) -> bool {
    !is_in_characters_folder(path) && !has_character_extension(below_data(path))
}

/// Returns whether `path` names a `.cdf`, `.chr` or `.skin` file in the characters folder, the
/// human body rigs included.
pub(crate) fn is_character_path(path: &str) -> bool {
    is_in_characters_folder(path) && has_character_extension(below_data(path))
}

/// Returns whether `path` names a character file outside the human body rigs that lies in the
/// characters folder: the creatures and other non-human characters, not the animated parts of
/// weapons, gadgets, ships and buildings.
pub(crate) fn is_creature_path(path: &str) -> bool {
    is_character_path(path) && !starts_with_path(below_data(path), HUMAN_RIGS)
}

#[cfg(test)]
mod tests {
    use super::{is_character_path, is_creature_path, is_world_prop_path};

    #[test]
    fn creatures_are_the_character_files_in_the_characters_folder_outside_the_human_rigs() {
        for path in [
            "Objects/Characters/Creatures/x/x_skel.chr",
            "Data\\Objects\\Characters\\Vanduul\\drone\\body.skin",
        ] {
            assert!(is_creature_path(path), "{path}");
        }
        for path in [
            "Objects/Characters/Human/male_v7/export/bhm_skeleton_v7.chr",
            "Objects/FPS_Weapons/Weapons_V7/x/rifle.cdf",
            "Objects/Spaceships/Thrusters/nozzle.cdf",
            "Objects/Characters/Creatures/x/x.cgf",
        ] {
            assert!(!is_creature_path(path), "{path}");
        }
    }

    #[test]
    fn a_world_prop_is_no_character_file_and_lies_outside_the_characters_folder() {
        for path in [
            "Objects/buildingsets/human/universal/prop/storage/box_cardboard/box_clothing_1_shirt_a.cgf",
            "Data\\Objects\\Test\\box.cgf",
        ] {
            assert!(is_world_prop_path(path), "{path}");
        }
        for path in [
            "Objects/Characters/Human/male_v7/clothing/x/m_x.skin",
            "Data\\objects\\characters\\human\\female_v2\\clothing\\f_x_prop.cgf",
            "Objects/Test/shirt.skin",
        ] {
            assert!(!is_world_prop_path(path), "{path}");
        }
    }

    #[test]
    fn character_paths_are_the_character_files_below_the_characters_folder() {
        for path in [
            "Objects/Characters/Creatures/x/x_skel.chr",
            "Data/Objects/Characters/Human/male_v7/export/bhm_skeleton_v7.chr",
            "objects\\characters\\human\\male_v7\\body\\m_body.SKIN",
            "Data\\Objects\\Characters\\Vanduul\\drone\\x.Cdf",
            "/Objects/Characters/Human/heads/male/head.skin",
        ] {
            assert!(is_character_path(path), "{path}");
        }
        for path in [
            "Objects/Characters/Human/male_v7/body/m_body.cgf",
            "Objects/Characters/Human/male_v7/body/skin",
            "Objects/FPS_Weapons/Weapons_V7/x/rifle.cdf",
            "Data/Objects/Spaceships/Thrusters/nozzle.chr",
            "Characters/Creatures/x/x.chr",
        ] {
            assert!(!is_character_path(path), "{path}");
        }
    }
}
