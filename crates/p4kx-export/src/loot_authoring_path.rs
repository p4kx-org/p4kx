// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

const VIRTUAL_ROOT: &str = "data/objectcontainers/";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "LootContainerLookupRepairKind", extend("x-p4kx-vocabularyClass" = "control"))]
pub(super) enum RepairKind {
    DriveQualified,
    ParentRelative,
    TimestampedAnnotation,
}

pub(super) struct AuthoringPath {
    pub(super) kind: RepairKind,
    pub(super) normalized: String,
}

impl AuthoringPath {
    pub(super) fn parse(literal: &str) -> Option<Self> {
        let normalized = literal.replace('\\', "/").to_ascii_lowercase();
        let (kind, remainder) = prefix_form(&normalized)?;
        let mut anchors = remainder.match_indices(VIRTUAL_ROOT).filter(|(at, _)| {
            let Some(prefix) = remainder.get(..*at) else {
                return false;
            };
            match kind {
                RepairKind::DriveQualified | RepairKind::ParentRelative => {
                    prefix.is_empty() || prefix.ends_with('/')
                }
                RepairKind::TimestampedAnnotation => prefix.ends_with(' '),
            }
        });
        let (at, _) = anchors.next()?;
        if anchors.next().is_some() {
            return None;
        }
        let (prefix, rooted) = remainder.split_at(at);
        if !valid_prefix(kind, prefix) || !valid_virtual_path(rooted) {
            return None;
        }
        Some(Self {
            kind,
            normalized: rooted.replace('/', "\\"),
        })
    }
}

fn prefix_form(normalized: &str) -> Option<(RepairKind, &str)> {
    match normalized.as_bytes() {
        [drive, b':', b'/', ..] if drive.is_ascii_alphabetic() => normalized
            .get(3..)
            .map(|path| (RepairKind::DriveQualified, path)),
        [b'.', b'.', b'/', ..] => Some((
            RepairKind::ParentRelative,
            normalized.trim_start_matches("../"),
        )),
        [b'[', ..] => {
            let (clock, rest) = normalized.strip_prefix('[')?.split_once(']')?;
            let annotation = rest.strip_prefix(' ')?;
            valid_clock(clock).then_some((RepairKind::TimestampedAnnotation, annotation))
        }
        _ => None,
    }
}

fn valid_prefix(kind: RepairKind, prefix: &str) -> bool {
    match kind {
        RepairKind::DriveQualified | RepairKind::ParentRelative => {
            let components = prefix.strip_suffix('/').unwrap_or(prefix);
            prefix.is_empty()
                || (!components.is_empty() && components.split('/').all(valid_component))
        }
        RepairKind::TimestampedAnnotation => {
            !prefix.trim().is_empty()
                && prefix.chars().all(|character| {
                    !character.is_control() && !matches!(character, '/' | '\\' | '[' | ']' | ':')
                })
        }
    }
}

fn valid_virtual_path(rooted: &str) -> bool {
    let Some(tail) = rooted.strip_prefix(VIRTUAL_ROOT) else {
        return false;
    };
    let filename = tail.rsplit('/').next().unwrap_or_default();
    filename
        .strip_suffix(".socpak")
        .is_some_and(|stem| !stem.is_empty())
        && tail.split('/').all(valid_component)
}

fn valid_component(component: &str) -> bool {
    !component.is_empty()
        && component != "."
        && component != ".."
        && component.chars().all(|character| {
            !character.is_control() && !matches!(character, ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
}

fn valid_clock(clock: &str) -> bool {
    let (delimited, meridiem) = match clock.split_once(' ') {
        None => (clock, false),
        Some((delimited, "am" | "pm")) => (delimited, true),
        Some(_) => return false,
    };
    let Some((hours, minutes)) = delimited.split_once(':') else {
        return false;
    };
    if !(1..=2).contains(&hours.len())
        || minutes.len() != 2
        || !hours
            .bytes()
            .chain(minutes.bytes())
            .all(|byte| byte.is_ascii_digit())
    {
        return false;
    }
    let (Ok(hours), Ok(minutes)) = (hours.parse::<u8>(), minutes.parse::<u8>()) else {
        return false;
    };
    minutes < 60
        && if meridiem {
            (1..=12).contains(&hours)
        } else {
            hours < 24
        }
}

#[cfg(test)]
mod tests {
    use super::{AuthoringPath, RepairKind};

    #[test]
    fn only_explicit_unambiguous_authoring_paths_expose_virtual_root_keys() {
        for (path, kind) in [
            (
                r"Q:\build\Data\ObjectContainers\Folder\Fixture.SOCPAK",
                RepairKind::DriveQualified,
            ),
            (
                "../../build/data/objectcontainers/folder/fixture.socpak",
                RepairKind::ParentRelative,
            ),
            (
                "[19:44] authoring message Data/ObjectContainers/folder/fixture.socpak",
                RepairKind::TimestampedAnnotation,
            ),
            (
                "[7:44 PM] authoring message  Data/ObjectContainers/folder/fixture.socpak",
                RepairKind::TimestampedAnnotation,
            ),
        ] {
            let parsed = AuthoringPath::parse(path).expect("one explicit virtual root");
            assert_eq!(parsed.kind, kind, "the checked prefix form becomes visible");
            assert_eq!(
                parsed.normalized, "data\\objectcontainers\\folder\\fixture.socpak",
                "use the complete virtual key"
            );
        }
    }

    #[test]
    fn malformed_or_multiply_anchored_authoring_text_has_no_repair() {
        for path in [
            "Q:build/Data/ObjectContainers/fixture.socpak",
            "Q://build/Data/ObjectContainers/fixture.socpak",
            "Q:/build/../Data/ObjectContainers/fixture.socpak",
            "Q:/build/MyData/ObjectContainers/fixture.socpak",
            "Q:/Data/ObjectContainers/Data/ObjectContainers/fixture.socpak",
            "Q:/Data/ObjectContainers/../fixture.socpak",
            "../../Data/ObjectContainers//fixture.socpak",
            "../../build/Data/ObjectContainers/fixture",
            "../../build/Data/ObjectContainers/.socpak",
            "[16:10 PM] label Data/ObjectContainers/fixture.socpak",
            "[24:10] label Data/ObjectContainers/fixture.socpak",
            "[12:99] label Data/ObjectContainers/fixture.socpak",
            "[7:1 PM] label Data/ObjectContainers/fixture.socpak",
            "[7:10 UTC] label Data/ObjectContainers/fixture.socpak",
            "[7:10] label: Data/ObjectContainers/fixture.socpak",
            "[7:10] label Data/ObjectContainers/fixture.socpak notes",
            "[7:10] label/Data/ObjectContainers/fixture.socpak",
            "[7:10] Data/ObjectContainers/fixture.socpak",
            "Unqualified Data/ObjectContainers/fixture.socpak",
        ] {
            assert!(
                AuthoringPath::parse(path).is_none(),
                "invalid grammar: {path}"
            );
        }
    }
}
