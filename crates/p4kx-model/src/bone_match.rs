// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::Bone;

/// How the bones of one skeleton map onto another's, matched by name.
///
/// Used to bind a skin to a skeleton that is not its own, such as a piece of armour to the body
/// skeleton it is worn on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoneMatch {
    map: Vec<u16>,
    unmatched: Vec<usize>,
}

impl BoneMatch {
    /// Matches each bone of `source` to the bone of `target` with the same name, ignoring ASCII
    /// case; the first of equal names wins. A bone `target` lacks takes the match of its nearest
    /// ancestor in `source` that has one, else bone 0, and is listed in [`Self::unmatched`].
    #[must_use]
    pub fn new(target: &[Bone], source: &[Bone]) -> Self {
        let mut by_name: HashMap<String, u16> = HashMap::with_capacity(target.len());
        for (index, bone) in target.iter().enumerate() {
            if let Ok(index) = u16::try_from(index) {
                by_name
                    .entry(bone.name.to_ascii_lowercase())
                    .or_insert(index);
            }
        }
        let found: Vec<Option<u16>> = source
            .iter()
            .map(|bone| by_name.get(&bone.name.to_ascii_lowercase()).copied())
            .collect();
        let mut map = Vec::with_capacity(source.len());
        let mut unmatched = Vec::new();
        for (index, hit) in found.iter().enumerate() {
            if let Some(hit) = hit {
                map.push(*hit);
                continue;
            }
            unmatched.push(index);
            map.push(inherited(source, &found, index).unwrap_or(0));
        }
        Self { map, unmatched }
    }

    /// Returns the bone of the target for the bone `source` of the source skeleton; bone 0 for an
    /// index the source lacks.
    #[must_use]
    pub fn target(&self, source: u16) -> u16 {
        self.map.get(usize::from(source)).copied().unwrap_or(0)
    }

    /// Returns the indices of the source bones the target skeleton has no bone of that name for.
    #[must_use]
    pub fn unmatched(&self) -> &[usize] {
        &self.unmatched
    }
}

fn inherited(source: &[Bone], found: &[Option<u16>], start: usize) -> Option<u16> {
    let mut at = source.get(start)?.parent;
    for _ in 0..source.len() {
        let index = at?;
        if let Some(Some(hit)) = found.get(index) {
            return Some(*hit);
        }
        at = source.get(index)?.parent;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::BoneMatch;
    use crate::{Bone, BoneTransform};

    fn bone(name: &str, parent: Option<usize>) -> Bone {
        Bone {
            name: name.to_owned(),
            parent,
            controller_id: 0,
            node: None,
            relative: BoneTransform::IDENTITY,
            world: BoneTransform::IDENTITY,
        }
    }

    #[test]
    fn bones_match_by_name_ignoring_case() {
        let target = [
            bone("Root", None),
            bone("Spine", Some(0)),
            bone("Head", Some(1)),
        ];
        let source = [bone("head", None), bone("ROOT", None)];
        let matched = BoneMatch::new(&target, &source);
        assert_eq!((matched.target(0), matched.target(1)), (2, 0));
        assert_eq!(matched.unmatched(), [] as [usize; 0]);
    }

    #[test]
    fn a_missing_bone_takes_its_nearest_matched_ancestor() {
        let target = [bone("root", None), bone("arm", Some(0))];
        let source = [
            bone("root", None),
            bone("arm", Some(0)),
            bone("sleeve_a", Some(1)),
            bone("sleeve_b", Some(2)),
            bone("lost", None),
        ];
        let matched = BoneMatch::new(&target, &source);
        assert_eq!(matched.target(2), 1);
        assert_eq!(matched.target(3), 1);
        assert_eq!(matched.target(4), 0);
        assert_eq!(matched.unmatched(), [2, 3, 4]);
    }

    #[test]
    fn a_parent_loop_does_not_hang() {
        let target = [bone("root", None)];
        let source = [bone("a", Some(1)), bone("b", Some(0))];
        let matched = BoneMatch::new(&target, &source);
        assert_eq!(matched.unmatched(), [0, 1]);
        assert_eq!(matched.target(0), 0);
    }
}
