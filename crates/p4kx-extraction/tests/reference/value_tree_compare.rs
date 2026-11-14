// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Comparison of a Record P4KX built with the one a reference tool wrote.

#![expect(
    clippy::float_cmp,
    reason = "numbers are compared exactly at the precision of the field; a margin would hide a smallest-step change"
)]

use std::fmt::Write as _;

use super::tally::Compared;
use super::value_tree::{Embedded, JsonNumber, Node, RecordDoc};

pub(crate) const WEAK_NULL: &str = "StarBreaker writes null for a weak pointer in a derived type";
pub(crate) const DANGLING_NULL: &str =
    "StarBreaker writes null for a Reference to a Record that does not exist";
pub(crate) const LENIENT_BOOL: &str = "StarBreaker reads any non-zero bool byte as true";
pub(crate) const NON_FINITE_NULL: &str = "StarBreaker writes null for a non-finite number";
pub(crate) const NON_FINITE_TEXT: &str =
    "StarBreaker writes a non-finite number as the text NaN, INF or -INF";

const EXACT_INTEGER: f64 = 9_007_199_254_740_992.0;

#[expect(
    clippy::cast_possible_truncation,
    reason = "narrowing to single precision is the comparison"
)]
const fn narrow(number: f64) -> f32 {
    number as f32
}

#[expect(
    clippy::cast_precision_loss,
    reason = "an integer beyond 2^53 is compared exactly before any float comparison"
)]
const fn widen(number: i128) -> f64 {
    number as f64
}

fn number_matches(ours: &Node, theirs: JsonNumber) -> bool {
    match (ours, theirs) {
        (Node::Int(number), JsonNumber::Signed(other)) => *number == i128::from(other),
        (Node::Int(number), JsonNumber::Unsigned(other)) => *number == i128::from(other),
        (Node::Int(number), JsonNumber::Float(other)) => {
            other.abs() < EXACT_INTEGER && other.fract() == 0.0 && widen(*number) == other
        }
        (Node::Single(number), JsonNumber::Float(other)) => *number == narrow(other),
        (Node::Single(number), JsonNumber::Signed(other)) => *number == narrow(widen(other.into())),
        (Node::Single(number), JsonNumber::Unsigned(other)) => {
            *number == narrow(widen(other.into()))
        }
        (Node::Double(number), JsonNumber::Float(other)) => *number == other,
        (Node::Double(number), JsonNumber::Signed(other)) => *number == widen(other.into()),
        (Node::Double(number), JsonNumber::Unsigned(other)) => *number == widen(other.into()),
        _ => false,
    }
}

/// Whether two Reference values name the same Record: by GUID when the reference side states
/// one, else by the record path against its relative `file://` path.
fn same_target(guid: &str, path: &str, other_guid: &str, other_path: &str) -> bool {
    if !other_guid.is_empty() {
        return guid == other_guid;
    }
    let mut relative = other_path.strip_prefix("file://").unwrap_or(other_path);
    while let Some(rest) = relative
        .strip_prefix("../")
        .or_else(|| relative.strip_prefix("./"))
    {
        relative = rest;
    }
    relative
        .strip_suffix(".json")
        .is_some_and(|stem| stem == path)
}

/// The text `StarBreaker` writes for a non-finite number, `None` for a finite one.
const fn non_finite_text(node: &Node) -> Option<&'static str> {
    let (finite, nan, positive) = match node {
        Node::Single(number) => (
            number.is_finite(),
            number.is_nan(),
            number.is_sign_positive(),
        ),
        Node::Double(number) => (
            number.is_finite(),
            number.is_nan(),
            number.is_sign_positive(),
        ),
        _ => return None,
    };
    if finite {
        None
    } else if nan {
        Some("NaN")
    } else if positive {
        Some("INF")
    } else {
        Some("-INF")
    }
}

/// Builds the Record P4KX has under a GUID; `None` when there is none.
pub(crate) type Resolve<'r> = dyn FnMut(&str) -> Option<RecordDoc> + 'r;

/// What a comparison saw besides the differences: the reason of each allowed value and the
/// number of embedded Records compared by content, and how to find the Record P4KX holds for an
/// embedded one.
struct Observed<'r> {
    allowed: Vec<&'static str>,
    embedded: u64,
    resolve: &'r mut Resolve<'r>,
}

/// Compares `ours` with the Record a reference tool wrote: type, GUID, name and every field by
/// name, kind and value. An embedded Record is a difference, as there is nothing to compare it
/// with.
#[must_use]
pub(crate) fn compare_records(ours: &RecordDoc, theirs: &RecordDoc) -> Compared {
    compare_counting(ours, theirs, &mut |_| None).0
}

/// As [`compare_records`], resolving the Record P4KX holds for each embedded Record with
/// `resolve` and comparing the two by content. Also returns how many embedded Records were
/// compared; when the Record differs, that is the number reached before the first difference.
#[must_use]
pub(crate) fn compare_counting(
    ours: &RecordDoc,
    theirs: &RecordDoc,
    resolve: &mut Resolve<'_>,
) -> (Compared, u64) {
    if ours.guid != theirs.guid {
        let difference = format!("GUID {} against {}", ours.guid, theirs.guid);
        return (Compared::Different(difference), 0);
    }
    if ours.name != theirs.name {
        let difference = format!("name {:?} against {:?}", ours.name, theirs.name);
        return (Compared::Different(difference), 0);
    }
    if ours.tag != theirs.tag {
        let difference = format!("tag {:?} against {:?}", ours.tag, theirs.tag);
        return (Compared::Different(difference), 0);
    }
    let mut seen = Observed {
        allowed: Vec::new(),
        embedded: 0,
        resolve,
    };
    let found = walk(&ours.root, &theirs.root, &mut String::new(), &mut seen);
    finish(found, seen)
}

/// Compares a manifest-selected embedded Record without inventing absent header fields.
pub(crate) fn compare_embedded_counting(
    guid: &str,
    content: &Embedded,
    resolve: &mut Resolve<'_>,
) -> (Compared, u64) {
    let mut seen = Observed {
        allowed: Vec::new(),
        embedded: 0,
        resolve,
    };
    let found = walk_embedded(guid, content, &mut String::new(), &mut seen);
    finish(found, seen)
}

fn finish(found: Option<String>, seen: Observed<'_>) -> (Compared, u64) {
    let compared = match found {
        Some(difference) => Compared::Different(difference),
        None if seen.allowed.is_empty() => Compared::Equal,
        None => Compared::Allowed(seen.allowed),
    };
    (compared, seen.embedded)
}
/// Compares the Record P4KX holds under `guid` with the content the reference embedded.
fn walk_embedded(
    guid: &str,
    content: &Embedded,
    path: &mut String,
    seen: &mut Observed<'_>,
) -> Option<String> {
    seen.embedded += 1;
    let Some(record) = (seen.resolve)(guid) else {
        return Some(format!(
            "{path}: no Record {guid} to compare the embedded one with"
        ));
    };
    if record.name != content.name {
        return Some(format!(
            "{path}: embedded name {:?} against {:?}",
            record.name, content.name
        ));
    }
    let length = path.len();
    let _written = write!(path, "->{}", content.name);
    let found = walk(&record.root, &content.root, path, seen);
    path.truncate(length);
    found
}

fn walk(ours: &Node, theirs: &Node, path: &mut String, seen: &mut Observed<'_>) -> Option<String> {
    let mismatch = |what: &str| Some(format!("{path}: {what}: {ours:?} against {theirs:?}"));
    match (ours, theirs) {
        (Node::Null, Node::Null) | (Node::WeakPointer, Node::WeakPointer) => None,
        (Node::WeakPointer, Node::Null) => allow(seen, WEAK_NULL),
        (Node::Dangling, Node::Null) => allow(seen, DANGLING_NULL),
        (Node::InvalidBool, Node::Bool(true)) => allow(seen, LENIENT_BOOL),
        (Node::Bool(a), Node::Bool(b)) if a == b => None,
        (Node::Text(a), Node::Text(b)) if a == b => None,
        (Node::Guid(a), Node::Guid(b) | Node::Text(b)) if a == b => None,
        (
            Node::Reference {
                guid,
                path: record_path,
                ..
            },
            Node::Reference {
                guid: other_guid,
                path: other_path,
                embedded,
            },
        ) if same_target(guid, record_path, other_guid, other_path) => embedded
            .as_deref()
            .and_then(|content| walk_embedded(guid, content, path, seen)),
        (Node::Single(_) | Node::Double(_), Node::Null) if non_finite_text(ours).is_some() => {
            allow(seen, NON_FINITE_NULL)
        }
        (Node::Single(_) | Node::Double(_), Node::Text(text))
            if non_finite_text(ours) == Some(text.as_str()) =>
        {
            allow(seen, NON_FINITE_TEXT)
        }
        (_, Node::Number(number)) if number_matches(ours, *number) => None,
        (
            Node::Struct {
                type_name: our_type,
                fields: our_fields,
            },
            Node::Struct {
                type_name: their_type,
                fields: their_fields,
            },
        ) => {
            if our_type != their_type {
                return mismatch("struct type");
            }
            walk_fields(our_fields, their_fields, path, seen)
        }
        (Node::Array(a), Node::Array(b)) => walk_elements(a, b, path, seen),
        _ => mismatch("value"),
    }
}

fn walk_elements(
    ours: &[Node],
    theirs: &[Node],
    path: &mut String,
    seen: &mut Observed<'_>,
) -> Option<String> {
    if ours.len() != theirs.len() {
        return Some(format!(
            "{path}: {} elements against {}",
            ours.len(),
            theirs.len()
        ));
    }
    ours.iter()
        .zip(theirs)
        .enumerate()
        .find_map(|(index, (x, y))| {
            let length = path.len();
            let _written = write!(path, "[{index}]");
            let found = walk(x, y, path, seen);
            path.truncate(length);
            found
        })
}

fn allow(seen: &mut Observed<'_>, reason: &'static str) -> Option<String> {
    seen.allowed.push(reason);
    None
}

fn walk_fields(
    ours: &[(String, Node)],
    theirs: &[(String, Node)],
    path: &mut String,
    seen: &mut Observed<'_>,
) -> Option<String> {
    if let Some(((name, _), (other, _))) = ours
        .iter()
        .zip(theirs)
        .find(|((name, _), (other, _))| name != other)
    {
        return Some(format!("{path}: field {name:?} against {other:?}"));
    }
    if ours.len() != theirs.len() {
        let (side, extra) = if ours.len() > theirs.len() {
            ("P4KX", ours.get(theirs.len()))
        } else {
            ("the reference", theirs.get(ours.len()))
        };
        let name = extra.map_or("", |(name, _)| name.as_str());
        return Some(format!("{path}: field {name:?} only in {side}"));
    }
    ours.iter().zip(theirs).find_map(|((name, x), (_, y))| {
        let length = path.len();
        path.push('.');
        path.push_str(name);
        let found = walk(x, y, path, seen);
        path.truncate(length);
        found
    })
}
