// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The one rule a file-name part of an Extraction or a Record export passes: valid on Linux and on
//! Windows alike, on every platform (ADR-0248).

use std::path::Path;

use thiserror::Error;

/// The most bytes in UTF-8 a name part may have (`NAME_MAX` of Linux file systems); the limit of
/// 255 UTF-16 units of NTFS is the same number and never the tighter one.
pub const MAX_PART_BYTES: usize = 255;

/// The longest file path in UTF-16 units, with its terminating NUL, that Windows accepts without
/// long-path support (`MAX_PATH`).
const FILE_PATH_UNITS: usize = 260;

/// The longest folder path in UTF-16 units, with its terminating NUL, that Windows creates
/// without long-path support.
const FOLDER_PATH_UNITS: usize = 248;

/// The rule a name part breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum NameRule {
    /// The part is empty.
    #[error("the name is empty")]
    Empty,
    /// The part is `.` or `..`.
    #[error("the name is a dot part")]
    DotPart,
    /// The part holds a character that Windows forbids in file names, or a control character.
    #[error("the character {0:?} is not allowed in a file name")]
    ForbiddenCharacter(char),
    /// The part ends with a dot or a space, which Windows drops.
    #[error("the name ends with a dot or a space")]
    TrailingDotOrSpace,
    /// The part is a Windows device name, with or without an extension.
    #[error("the name is a Windows device name")]
    DeviceName,
    /// The part is reserved for an origin publication lock.
    #[error("the name is reserved for the origin publication lock")]
    ReservedOriginLock,
    /// The part is longer than [`MAX_PART_BYTES`] in UTF-8, which also bounds it in UTF-16 units.
    #[error("the name is {0} bytes in UTF-8, more than {MAX_PART_BYTES}")]
    TooManyBytes(usize),
}

/// A path longer than Windows opens or creates without long-path support.
///
/// The limits of 260 UTF-16 units for a file and 248 for a folder are tighter than the 4096 bytes
/// (`PATH_MAX`) of Linux, which a path of at most 260 units, at most four bytes each, never
/// reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("the path is {units} UTF-16 units with its end, more than the {limit} Windows allows")]
pub struct PathLimit {
    /// The length in UTF-16 units, counting the terminating NUL.
    pub units: usize,
    /// The limit that was exceeded.
    pub limit: usize,
}

/// Returns the first rule the name part `part` breaks, or `Ok` when it is a valid file name on
/// Linux and Windows; runs in time linear in the length of `part` and allocates nothing.
///
/// # Errors
///
/// Returns the rule `part` breaks.
pub fn check_part(part: &str) -> Result<(), NameRule> {
    if part.is_empty() {
        return Err(NameRule::Empty);
    }
    if part == "." || part == ".." {
        return Err(NameRule::DotPart);
    }
    if let Some(character) = part.chars().find(|&character| is_forbidden(character)) {
        return Err(NameRule::ForbiddenCharacter(character));
    }
    if part.ends_with(['.', ' ']) {
        return Err(NameRule::TrailingDotOrSpace);
    }
    if is_device_name(part) {
        return Err(NameRule::DeviceName);
    }
    check_length(part)
}

/// Checks the length of a part. A character takes at least as many UTF-8 bytes as UTF-16 units,
/// so the limit of 255 bytes also keeps a part within 255 UTF-16 units.
pub(crate) const fn check_length(part: &str) -> Result<(), NameRule> {
    if part.len() > MAX_PART_BYTES {
        return Err(NameRule::TooManyBytes(part.len()));
    }
    Ok(())
}

/// Returns whether Windows forbids `character` in a file name, or it is a control character.
pub(crate) const fn is_forbidden(character: char) -> bool {
    matches!(
        character,
        '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'..='\u{1F}'
    )
}

/// Returns whether `part`, ignoring an extension, is a Windows device name.
pub(crate) fn is_device_name(part: &str) -> bool {
    const NAMES: [&str; 6] = ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"];
    let stem = part.split('.').next().unwrap_or(part).trim_end();
    if NAMES.iter().any(|name| stem.eq_ignore_ascii_case(name)) {
        return true;
    }
    let Some(prefix) = stem.get(..3) else {
        return false;
    };
    if !(prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT")) {
        return false;
    }
    let mut number = stem.get(3..).unwrap_or_default().chars();
    matches!(
        (number.next(), number.next()),
        (Some('0'..='9' | '\u{B9}' | '\u{B2}' | '\u{B3}'), None)
    )
}

/// Checks that the absolute file path `file` and its folder stay within the path limits of
/// Windows, and so of Linux.
///
/// # Errors
///
/// Returns the [`PathLimit`] the path or its folder exceeds.
pub fn check_path_length(file: &Path) -> Result<(), PathLimit> {
    check_units(file, FILE_PATH_UNITS)?;
    file.parent()
        .map_or(Ok(()), |folder| check_units(folder, FOLDER_PATH_UNITS))
}

fn check_units(path: &Path, limit: usize) -> Result<(), PathLimit> {
    let units = path
        .as_os_str()
        .to_string_lossy()
        .chars()
        .map(char::len_utf16)
        .sum::<usize>()
        + 1;
    if units > limit {
        return Err(PathLimit { units, limit });
    }
    Ok(())
}

/// Returns `path` folded the way a case-insensitive file system compares names: Unicode simple
/// lower case per character, with `/` for `\`. NTFS compares through its own upper-case table,
/// which differs for a few characters such as dotless `ı`, so two names this folds apart can
/// still collide there.
pub(crate) fn fold(path: &str) -> impl Iterator<Item = char> + '_ {
    path.chars()
        .flat_map(char::to_lowercase)
        .map(|c| if c == '\\' { '/' } else { c })
}

/// Returns `name` with every character in Unicode simple lower case and `\` as `/`: the key two
/// names share when they would be the same file on a case-insensitive file system.
#[must_use]
pub fn fold_name(name: &str) -> String {
    fold(name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_forbidden_character_is_named() {
        for character in "<>:\"/\\|?*".chars().chain('\0'..='\u{1F}') {
            let part = format!("a{character}b");
            assert_eq!(
                check_part(&part),
                Err(NameRule::ForbiddenCharacter(character)),
                "{part:?}"
            );
        }
    }

    #[test]
    fn plain_names_pass() {
        for part in [
            "a.txt",
            "..a",
            "b..c.xml",
            "com10.txt",
            "Straße",
            "a b",
            "%41",
            "\u{7F}",
            "co",
            "é",
        ] {
            assert_eq!(check_part(part), Ok(()), "{part:?}");
        }
    }

    #[test]
    fn structural_names_fail_by_rule() {
        assert_eq!(check_part(""), Err(NameRule::Empty));
        assert_eq!(check_part("."), Err(NameRule::DotPart));
        assert_eq!(check_part(".."), Err(NameRule::DotPart));
        assert_eq!(check_part("a."), Err(NameRule::TrailingDotOrSpace));
        assert_eq!(check_part("a "), Err(NameRule::TrailingDotOrSpace));
        assert_eq!(check_part("..."), Err(NameRule::TrailingDotOrSpace));
    }

    #[test]
    fn device_names_fail_with_and_without_extension() {
        for part in [
            "NUL",
            "nul.txt",
            "Con.tar.gz",
            "aux .txt",
            "COM1",
            "lpt9.log",
            "COM\u{B9}",
            "LPT\u{B3}.x",
            "CONOUT$",
            "com0",
        ] {
            assert_eq!(check_part(part), Err(NameRule::DeviceName), "{part:?}");
        }
        assert_eq!(check_part("COM10"), Ok(()));
        assert_eq!(check_part("console"), Ok(()));
    }

    #[test]
    fn the_length_limits_count_utf8_bytes_and_utf16_units() {
        assert_eq!(check_part(&"a".repeat(255)), Ok(()));
        assert_eq!(
            check_part(&"a".repeat(256)),
            Err(NameRule::TooManyBytes(256))
        );
        assert_eq!(check_part(&"ä".repeat(127)), Ok(()));
        assert_eq!(
            check_part(&"ä".repeat(128)),
            Err(NameRule::TooManyBytes(256))
        );
        assert_eq!(check_part(&"\u{20AC}".repeat(85)), Ok(()));
        assert_eq!(
            check_part(&"\u{20AC}".repeat(86)),
            Err(NameRule::TooManyBytes(258))
        );
        assert_eq!(check_part(&"\u{1F600}".repeat(63)), Ok(()));
        assert_eq!(
            check_part(&"\u{1F600}".repeat(64)),
            Err(NameRule::TooManyBytes(256))
        );
    }

    #[test]
    fn a_part_within_the_byte_limit_is_within_the_utf16_limit() {
        let units = |part: &str| part.chars().map(char::len_utf16).sum::<usize>();
        for part in [
            "a".repeat(255),
            "ä".repeat(127),
            "\u{20AC}".repeat(85),
            "\u{1F600}".repeat(63),
        ] {
            assert_eq!(check_part(&part), Ok(()));
            assert!(units(&part) <= 255, "{} units", units(&part));
        }
    }

    #[test]
    fn a_file_path_over_260_units_or_a_folder_over_248_is_named() {
        let short = Path::new("out").join("a").join("b");
        assert_eq!(check_path_length(&short), Ok(()));
        let long_file = Path::new(&"a".repeat(100))
            .join("b".repeat(100))
            .join("c".repeat(57));
        assert_eq!(long_file.as_os_str().len(), 259);
        assert_eq!(check_path_length(&long_file), Ok(()));
        assert_eq!(
            check_path_length(&long_file.join("d")),
            Err(PathLimit {
                units: 262,
                limit: 260
            })
        );
        let deep_folder = Path::new(&"a".repeat(120)).join("b".repeat(127)).join("f");
        assert_eq!(
            check_path_length(&deep_folder),
            Err(PathLimit {
                units: 120 + 1 + 127 + 1,
                limit: 248
            })
        );
    }

    #[test]
    fn folding_lowers_case_and_unifies_separators() {
        assert_eq!(fold("Data\\Ä/X").collect::<String>(), "data/ä/x");
        assert_eq!(fold_name("Play_A"), fold_name("PLAY_a"));
    }
}
