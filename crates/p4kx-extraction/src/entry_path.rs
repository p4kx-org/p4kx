// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsStr;
use std::path::PathBuf;

use crate::file_receipt::is_origin_lock_name;
use crate::part_escape::hex_digit;
use crate::portable_name::check_part;
use crate::{ExtractionError, NameRule};

/// Returns the path below the target an Entry is written to, or the part that is no valid file
/// extraction name and the rule it breaks ([`ExtractionError::InvalidName`]).
pub(crate) fn relative_path(entry_path: &str) -> Result<PathBuf, ExtractionError> {
    let mut relative = PathBuf::new();
    for part in entry_path.split(['\\', '/']) {
        check_output_part(part)?;
        relative.push(part);
    }
    Ok(relative)
}

/// Checks a portable part against the extraction output namespace.
pub(crate) fn check_output_part(part: &str) -> Result<(), ExtractionError> {
    let rule = if is_origin_lock_name(OsStr::new(part)) {
        Err(NameRule::ReservedOriginLock)
    } else {
        check_part(part)
    };
    rule.map_err(|rule| ExtractionError::InvalidName {
        part: part.to_owned(),
        rule,
    })
}

/// Returns the Entry path to extract for stored path bytes that are not UTF-8: the valid text as
/// it is, `%` as `%25` and every invalid byte as `%` and two upper-case hex digits.
///
/// Different byte strings never give the same name, which the lossy path of the Entry does not
/// guarantee; [`relative_path`] still checks the result.
pub(crate) fn escaped_raw_path(raw: &[u8]) -> String {
    let mut path = String::with_capacity(raw.len());
    for chunk in raw.utf8_chunks() {
        for character in chunk.valid().chars() {
            if character == '%' {
                path.push_str("%25");
            } else {
                path.push(character);
            }
        }
        for byte in chunk.invalid() {
            path.extend(['%', hex_digit(byte >> 4), hex_digit(byte & 0x0F)]);
        }
    }
    path
}

/// Returns whether two Entry paths are equal when ASCII case and the separator kind are ignored.
pub(crate) fn same_entry_path(left: &str, right: &str) -> bool {
    let separator = |byte: u8| byte == b'\\' || byte == b'/';
    left.len() == right.len()
        && left
            .bytes()
            .zip(right.bytes())
            .all(|(a, b)| a.eq_ignore_ascii_case(&b) || (separator(a) && separator(b)))
}

#[cfg(test)]
mod tests {
    use super::{escaped_raw_path, relative_path};
    use crate::{ExtractionError, NameRule};
    use std::path::PathBuf;

    #[test]
    fn raw_paths_escape_invalid_bytes_and_percent_signs_without_collisions() {
        assert_eq!(escaped_raw_path(b"Data\\caf\xE9.txt"), "Data\\caf%E9.txt");
        assert_eq!(escaped_raw_path(b"a\xFF\xFE"), "a%FF%FE");
        assert_eq!(escaped_raw_path(b"a%E9\xC3"), "a%25E9%C3");
        assert_ne!(escaped_raw_path(b"a\xE9"), escaped_raw_path(b"a\xE8"));
        assert_ne!(
            escaped_raw_path(b"a\xE9"),
            escaped_raw_path(b"a%E9\xC3\xA9"),
            "a literal percent sign cannot imitate an escape"
        );
    }

    #[test]
    fn escaped_raw_paths_that_traverse_are_still_rejected() {
        assert!(relative_path(&escaped_raw_path(b"..\\\xFF\\..")).is_err());
        assert!(relative_path(&escaped_raw_path(b"a\\..")).is_err());
    }

    #[test]
    fn plain_entry_paths_map_to_nested_relative_paths() {
        assert_eq!(
            relative_path("Data\\Libs/a.xml").ok(),
            Some(["Data", "Libs", "a.xml"].iter().collect::<PathBuf>()),
            "both separators split the path"
        );
    }

    #[test]
    fn names_with_inner_dots_are_plain() {
        assert!(
            relative_path("Data\\..a\\b..c.xml").is_ok(),
            "dots inside a name are fine"
        );
        assert!(
            relative_path("console\\com10.txt").is_ok(),
            "names that only start like a device are fine"
        );
    }

    #[test]
    fn invalid_names_are_rejected_with_their_rule() {
        let cases = [
            ("..", NameRule::DotPart),
            ("a\\..\\..\\b", NameRule::DotPart),
            (".\\a", NameRule::DotPart),
            ("\\a", NameRule::Empty),
            ("a\\\\b", NameRule::Empty),
            ("", NameRule::Empty),
            ("a\\NUL", NameRule::DeviceName),
            ("nul.txt", NameRule::DeviceName),
            ("COM\u{B9}", NameRule::DeviceName),
            ("CONOUT$", NameRule::DeviceName),
            ("C:\\a", NameRule::ForbiddenCharacter(':')),
            ("a\\b:stream", NameRule::ForbiddenCharacter(':')),
            ("a\\b<c", NameRule::ForbiddenCharacter('<')),
            ("a\\\0b", NameRule::ForbiddenCharacter('\0')),
            ("a\\b.", NameRule::TrailingDotOrSpace),
            ("a\\b ", NameRule::TrailingDotOrSpace),
        ];
        for (path, expected) in cases {
            assert!(
                matches!(
                    relative_path(path),
                    Err(ExtractionError::InvalidName { rule, .. }) if rule == expected
                ),
                "{path:?} breaks {expected:?}"
            );
        }
    }
}
