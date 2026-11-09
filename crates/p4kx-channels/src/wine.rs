// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

pub use crate::wine_environment::WineEnvironment;

pub(crate) fn game_library(prefix: &Path) -> PathBuf {
    prefix.join("drive_c/Program Files/Roberts Space Industries/StarCitizen")
}

pub(crate) fn append_parts(mut base: PathBuf, rest: &str) -> Option<PathBuf> {
    for part in rest.split(['\\', '/']).filter(|part| !part.is_empty()) {
        match part {
            "." => {}
            ".." => return None,
            _ if part.contains(':') => return None,
            _ => base.push(part),
        }
    }
    Some(base)
}

pub(crate) fn split_drive(text: &str) -> Option<(char, &str)> {
    let mut chars = text.chars();
    let drive = chars.next().filter(char::is_ascii_alphabetic)?;
    if chars.next()? != ':' {
        return None;
    }
    let rest = chars.as_str();
    if !rest.is_empty() && !rest.starts_with(['\\', '/']) {
        return None;
    }
    Some((drive.to_ascii_lowercase(), rest))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{append_parts, split_drive};

    fn host_path(prefix: &Path, path: &Path) -> Option<PathBuf> {
        let (drive, rest) = split_drive(path.to_str()?)?;
        if drive != 'c' {
            return None;
        }
        append_parts(prefix.join("drive_c"), rest)
    }

    fn prefix() -> PathBuf {
        PathBuf::from("prefix")
    }

    #[test]
    fn c_drive_paths_map_below_drive_c() {
        let host = host_path(&prefix(), Path::new(r"C:\Program Files\Game\LIVE"));
        let expected = prefix()
            .join("drive_c")
            .join("Program Files")
            .join("Game")
            .join("LIVE");
        assert_eq!(host, Some(expected));
    }

    #[test]
    fn drive_relative_paths_are_refused() {
        for path in ["C:Games", "D:Games"] {
            assert_eq!(
                split_drive(path),
                None,
                "a drive letter does not make a relative path absolute"
            );
        }
    }

    #[test]
    fn forward_slashes_and_lower_case_drives_map_too() {
        let host = host_path(&prefix(), Path::new("c:/Games//SC/./LIVE"));
        let expected = prefix()
            .join("drive_c")
            .join("Games")
            .join("SC")
            .join("LIVE");
        assert_eq!(host, Some(expected));
    }

    #[test]
    fn a_path_leaving_the_drive_is_refused() {
        assert_eq!(host_path(&prefix(), Path::new(r"C:\Games\..\..\etc")), None);
    }

    #[test]
    fn paths_without_a_drive_letter_are_refused() {
        for path in [
            "/data/games",
            "games/SC",
            r"\\server\share\SC",
            r"\\?\C:\Games\SC",
            r"\\?\UNC\server\share",
        ] {
            assert_eq!(host_path(&prefix(), Path::new(path)), None, "{path}");
        }
    }
}
