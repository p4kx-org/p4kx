// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::VecDeque;

use p4kx_extraction::ArchiveWatch;
use p4kx_extraction::cryxml::{SourceDeclarationKind, SourceElement};

use super::super::chrparams_selectors::{self, Target};
use super::{TestResult, require};
use crate::MediaError;

#[derive(Default)]
pub(super) struct Counts {
    pub(super) declarations: usize,
    pub(super) wildcards: usize,
    pub(super) folders: usize,
    pub(super) malformed_folders: usize,
    pub(super) resets: usize,
    pub(super) targets: usize,
    pub(super) default_pose_names: usize,
}

#[derive(Default)]
struct LexicalAnimation<'source> {
    name: Option<&'source str>,
    path: Option<&'source str>,
    ambiguous: bool,
    folder: bool,
}

#[derive(PartialEq, Eq)]
enum Kind {
    Definition,
    Reference,
}

pub(super) fn compare(
    bytes: &[u8],
    root: SourceElement<'_>,
    counts: &mut Counts,
    watch: &ArchiveWatch,
) -> TestResult {
    let mut expected = expected_targets(root, counts)?;
    counts.targets += expected.len();
    chrparams_selectors::visit_targets(
        bytes,
        |_| {
            watch.poll()?;
            Ok(())
        },
        |target| {
            let observed = match target {
                Target::Definition(path) => (Kind::Definition, path),
                Target::Reference(path) => (Kind::Reference, path),
            };
            if expected.pop_front().as_ref() != Some(&observed) {
                return Err(MediaError::Contract(
                    "ordered scoped target differs from source syntax".into(),
                ));
            }
            Ok(())
        },
    )?;
    require(
        expected.is_empty(),
        "all expected ordered target occurrences emitted",
    )
}

fn expected_targets(
    root: SourceElement<'_>,
    counts: &mut Counts,
) -> TestResult<VecDeque<(Kind, String)>> {
    let mut expected = VecDeque::new();
    if !name_is(root, "Params") {
        return Ok(expected);
    }
    for list in root
        .elements()
        .filter(|element| name_is(*element, "AnimationList"))
    {
        let mut folder = None;
        for element in list
            .elements()
            .filter(|element| name_is(*element, "Animation"))
        {
            counts.declarations += 1;
            let authored = lexical(element)?;
            if authored.folder {
                counts.folders += 1;
                folder = if authored.ambiguous || authored.path.is_none() {
                    counts.malformed_folders += 1;
                    None
                } else {
                    authored.path
                };
                counts.resets += usize::from(
                    folder.is_some_and(|path| path.trim_end_matches(['/', '\\']).is_empty()),
                );
                continue;
            }
            if !authored.ambiguous
                && let (Some(name), Some(path)) = (authored.name, authored.path)
            {
                counts.default_pose_names += usize::from(default_pose_spelling(name));
                counts.wildcards += usize::from(pattern(name, path));
                if let Some(target) = expected_target(name, path, folder) {
                    expected.try_reserve(1)?;
                    expected.push_back(target);
                }
            }
        }
    }
    Ok(expected)
}

fn lexical(element: SourceElement<'_>) -> TestResult<LexicalAnimation<'_>> {
    let mut authored = LexicalAnimation::default();
    for declaration in element.syntax()?.declarations() {
        let declaration = declaration?;
        if !matches!(
            declaration.kind(),
            SourceDeclarationKind::Attribute { namespace: None }
        ) {
            continue;
        }
        if declaration.qualified_name().eq_ignore_ascii_case("name") {
            authored.folder |= declaration.value().eq_ignore_ascii_case("#filepath");
            authored.ambiguous |= authored.name.replace(declaration.value()).is_some();
        } else if declaration.qualified_name().eq_ignore_ascii_case("path") {
            authored.ambiguous |= authored.path.replace(declaration.value()).is_some();
        }
    }
    Ok(authored)
}

fn expected_target(name: &str, path: &str, folder: Option<&str>) -> Option<(Kind, String)> {
    let extension = path.rsplit_once('.')?.1;
    if name.eq_ignore_ascii_case("$AnimEventDatabase") {
        return extension
            .eq_ignore_ascii_case("animevents")
            .then(|| rooted(path).map(|path| (Kind::Definition, path)))
            .flatten();
    }
    if name.eq_ignore_ascii_case("$TracksDatabase") {
        return extension
            .eq_ignore_ascii_case("dba")
            .then(|| rooted(path).map(|path| (Kind::Reference, path)))
            .flatten();
    }
    if name.is_empty()
        || name.starts_with(['#', '$'])
        || pattern(name, path)
        || !extension.eq_ignore_ascii_case("caf")
    {
        return None;
    }
    let root = path.split_once(['/', '\\']).map(|(root, _)| root);
    let rooted_selector = root.is_some_and(|root| {
        root.eq_ignore_ascii_case("Data") || root.eq_ignore_ascii_case("Animations")
    });
    if rooted_selector {
        return rooted(path).map(|path| (Kind::Reference, path));
    }
    let folder = folder?.trim_end_matches(['/', '\\']);
    if folder.is_empty() {
        return None;
    }
    rooted(&format!("{folder}/{path}")).map(|path| (Kind::Reference, path))
}

fn rooted(path: &str) -> Option<String> {
    let portable = path.replace('\\', "/");
    let (root, rest) = portable.split_once('/')?;
    let valid = rest.split('/').all(|segment| {
        !segment.is_empty()
            && !segment.ends_with([' ', '.'])
            && !segment.contains([':', '\0', '!', '*', '?'])
    });
    if !valid {
        return None;
    }
    if root.eq_ignore_ascii_case("Data") {
        Some(portable)
    } else if root.eq_ignore_ascii_case("Animations") {
        Some(format!("Data/{portable}"))
    } else {
        None
    }
}

fn name_is(element: SourceElement<'_>, name: &str) -> bool {
    element.namespace().is_none() && element.name().eq_ignore_ascii_case(name)
}

fn pattern(name: &str, path: &str) -> bool {
    !name.is_empty()
        && !path.is_empty()
        && !name.starts_with(['#', '$'])
        && (name.contains(['*', '?']) || path.contains(['*', '?']))
}

pub(super) fn default_pose_spelling(name: &str) -> bool {
    ["defaultpose", "default_pose", "default-pose"]
        .into_iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(name))
}
