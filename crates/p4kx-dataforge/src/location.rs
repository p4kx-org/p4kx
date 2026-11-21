// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::{self, Write as _};
use std::hash::{Hash, Hasher};

/// One step from a struct down to a value: a field by name or an element of an array by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step<'f> {
    /// The field of this name.
    Field(&'f str),
    /// The element at this index of the array reached so far.
    Element(u32),
}

/// A place inside a Record: the fields and array elements from the Record down to a value.
///
/// Its text names the fields separated by `.`, with `[index]` after an array field, e.g.
/// `scene[1].parent`.
#[derive(Clone)]
pub struct Location<'f> {
    steps: Box<[Step<'f>]>,
    text: Box<str>,
    original_positions: Option<Box<[u64]>>,
}

impl<'f> Location<'f> {
    /// Creates the location of `steps`.
    #[must_use]
    pub fn new(steps: &[Step<'f>]) -> Self {
        let mut text = String::new();
        for step in steps {
            match step {
                Step::Field(name) => {
                    if !text.is_empty() {
                        text.push('.');
                    }
                    text.push_str(name);
                }
                Step::Element(index) => {
                    let _infallible = write!(text, "[{index}]");
                }
            }
        }
        Self {
            steps: steps.into(),
            text: text.into(),
            original_positions: None,
        }
    }

    pub(crate) fn original_instance(steps: &[Step<'f>], positions: &[u64]) -> Self {
        Self {
            original_positions: Some(positions.into()),
            ..Self::new(steps)
        }
    }

    pub(crate) fn original_positions(&self) -> Option<&[u64]> {
        self.original_positions.as_deref()
    }

    /// Returns the steps from the Record down.
    #[must_use]
    pub fn steps(&self) -> &[Step<'f>] {
        &self.steps
    }

    /// Returns the text form, e.g. `scene[1].parent`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl PartialEq for Location<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.steps() == other.steps() && self.as_str() == other.as_str()
    }
}

impl Eq for Location<'_> {}

impl Hash for Location<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.steps().hash(state);
        self.as_str().hash(state);
    }
}

impl fmt::Debug for Location<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Location")
            .field("steps", &self.steps())
            .field("text", &self.as_str())
            .finish()
    }
}

impl fmt::Display for Location<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{Location, Step};

    #[test]
    fn original_instance_storage_keeps_wide_field_positions_without_changing_public_equality() {
        const WIDE_FIELD: u64 = 4_294_967_296;
        let steps = [Step::Field("late"), Step::Element(7)];
        let actual = Location::original_instance(&steps, &[WIDE_FIELD, 7]);
        assert_eq!(
            actual.original_positions(),
            Some([WIDE_FIELD, 7].as_slice()),
            "private original tuples retain the complete field ordinal"
        );
        let logical = Location::new(&steps);
        assert_eq!(
            actual, logical,
            "public Location equality remains based on semantic steps and text"
        );
        assert_eq!(
            logical.original_positions(),
            None,
            "a logical descriptor grants no original positions"
        );
    }

    #[test]
    fn public_location_constructor_has_no_original_instance_receipt() {
        let location = Location::new(&[Step::Field("child"), Step::Element(1)]);
        assert_eq!(
            location.as_str(),
            "child[1]",
            "the logical value is retained"
        );
        assert_eq!(
            location.original_positions(),
            None,
            "a public logical coordinate does not mint original physical positions"
        );
    }
}
