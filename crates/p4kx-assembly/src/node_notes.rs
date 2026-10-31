// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::LoadoutError;

/// Splits what resolving an Entity found into its issues and its notes, in order.
///
/// The notes of one assortment are merged into one, its undecided groups collected without
/// duplicates, so an assortment named twice is noted once.
pub(crate) fn split(found: Vec<LoadoutError>) -> (Vec<LoadoutError>, Vec<LoadoutError>) {
    let mut issues = Vec::with_capacity(found.len());
    let mut notes: Vec<LoadoutError> = Vec::new();
    for item in found {
        if !item.is_note() {
            issues.push(item);
            continue;
        }
        let LoadoutError::AssortmentRequirement {
            guid,
            assortment,
            groups,
        } = item
        else {
            continue;
        };
        let known = notes.iter_mut().find_map(|note| match note {
            LoadoutError::AssortmentRequirement {
                guid: known,
                assortment: known_assortment,
                groups: known_groups,
            } if *known == guid => Some((known_assortment, known_groups)),
            _ => None,
        });
        match known {
            Some((known_assortment, known_groups)) => {
                *known_assortment |= assortment;
                for group in groups {
                    if !known_groups.contains(&group) {
                        known_groups.push(group);
                    }
                }
            }
            None => notes.push(LoadoutError::AssortmentRequirement {
                guid,
                assortment,
                groups,
            }),
        }
    }
    (issues, notes)
}
