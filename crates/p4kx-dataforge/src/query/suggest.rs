// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// The longest name compared by edit distance; longer names are only compared ignoring case.
const MAX_COMPARED_LEN: usize = 128;

/// Returns the candidate closest to `name`: one equal ignoring ASCII case, otherwise the first
/// with the smallest edit distance up to a third of the name's length, at least 1.
pub(crate) fn closest<'c>(name: &str, candidates: impl Iterator<Item = &'c str>) -> Option<String> {
    match closest_fallible(name, candidates.map(Ok::<_, std::convert::Infallible>)) {
        Ok(suggestion) => suggestion,
        Err(error) => match error {},
    }
}

/// Returns the closest readable candidate, propagating a failed candidate traversal.
pub(crate) fn closest_fallible<'c, E>(
    name: &str,
    candidates: impl Iterator<Item = Result<&'c str, E>>,
) -> Result<Option<String>, E> {
    let wanted: Vec<char> = name.chars().take(MAX_COMPARED_LEN + 1).collect();
    let limit = (wanted.len() / 3).max(1);
    let mut best: Option<(usize, &str)> = None;
    let mut row = Vec::new();
    for candidate in candidates {
        let candidate = candidate?;
        if candidate == name {
            continue;
        }
        if candidate.eq_ignore_ascii_case(name) {
            return Ok(Some(candidate.to_owned()));
        }
        if wanted.len() > MAX_COMPARED_LEN || candidate.len() > MAX_COMPARED_LEN * 4 {
            continue;
        }
        let bound = best.map_or(limit, |(distance, _)| distance.saturating_sub(1));
        if let Some(distance) = distance(&wanted, candidate, bound, &mut row) {
            best = Some((distance, candidate));
        }
    }
    Ok(best.map(|(_, candidate)| candidate.to_owned()))
}

/// Returns the Levenshtein distance between `left` and `right` when it is at most `bound`.
fn distance(left: &[char], right: &str, bound: usize, row: &mut Vec<usize>) -> Option<usize> {
    if right.chars().count().abs_diff(left.len()) > bound {
        return None;
    }
    row.clear();
    row.extend(0..=left.len());
    for (j, right_char) in right.chars().enumerate() {
        let mut diagonal = j;
        let mut previous = j.checked_add(1)?;
        let first = row.first_mut()?;
        *first = previous;
        let mut smallest = previous;
        for (i, &left_char) in left.iter().enumerate() {
            let cell = i.checked_add(1)?;
            let above = *row.get(cell)?;
            let cost = usize::from(left_char != right_char);
            let current = diagonal
                .saturating_add(cost)
                .min(above.saturating_add(1))
                .min(previous.saturating_add(1));
            diagonal = above;
            previous = current;
            smallest = smallest.min(current);
            *row.get_mut(cell)? = current;
        }
        if smallest > bound {
            return None;
        }
    }
    row.last().copied().filter(|distance| *distance <= bound)
}
