// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Debug, Display};

/// A physical fixture identity and its exact source class.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Candidate<I> {
    /// The stable deterministic key, including physical source position where required.
    pub identity: I,
    /// The exact source class used for preferred matching.
    pub class_name: String,
}

impl<I> Candidate<I> {
    /// Creates one exact source candidate without deriving or merging its identity.
    #[must_use]
    pub const fn new(identity: I, class_name: String) -> Self {
        Self {
            identity,
            class_name,
        }
    }
}
/// Why one source identity represents a test case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectionReason {
    /// A documented preferred class is present.
    Preferred,
    /// Every preferred class is absent and an eligible capable source was selected.
    Fallback,
}

/// A selected source together with the capabilities its actual probe established.
#[derive(Debug)]
#[non_exhaustive]
pub struct Choice<I, C> {
    /// The exact selected physical source.
    pub candidate: Candidate<I>,
    /// The capabilities established before conversion.
    pub capabilities: C,
    /// The selection policy that chose this source.
    pub reason: SelectionReason,
}

/// A present fixture cannot safely represent its case.
#[derive(Debug)]
#[non_exhaustive]
pub enum SelectionError<I> {
    /// One physical identity occurs more than once in the candidate inventory.
    DuplicateIdentity(I),
    /// A preferred class resolves to multiple physical sources.
    AmbiguousPreferred {
        /// The ambiguous preferred class.
        class_name: String,
        /// Every exact matching identity, in deterministic order.
        identities: Vec<I>,
    },
    /// A present preferred source lacks required declarations or capabilities.
    IncompatiblePreferred {
        /// The actual source identity.
        identity: I,
    },
    /// An actual source read or capability probe failed.
    Probe {
        /// The actual source identity.
        identity: I,
        /// The original producer error.
        source: Box<dyn Error>,
    },
}

impl<I: Debug> Display for SelectionError<I> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::DuplicateIdentity(_) => "duplicate_physical_fixture_identity",
            Self::AmbiguousPreferred { .. } => "ambiguous_preferred_class",
            Self::IncompatiblePreferred { .. } => "preferred_capability_mismatch",
            Self::Probe { .. } => "fixture_probe_failure",
        })
    }
}
impl<I: Debug> Error for SelectionError<I> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Probe { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

/// Chooses an exact preference or the first eligible capable source in identity order.
///
/// # Errors
///
/// Returns an error for ambiguous or incompatible present preferences and actual probe errors.
pub fn select<I: Clone + Ord, C>(
    preferred: &[&str],
    candidates: &[Candidate<I>],
    eligible: impl Fn(&Candidate<I>) -> bool,
    mut probe: impl FnMut(&Candidate<I>) -> Result<Option<C>, Box<dyn Error>>,
) -> Result<Option<Choice<I, C>>, SelectionError<I>> {
    let mut identities = BTreeSet::new();
    for candidate in candidates {
        if !identities.insert(&candidate.identity) {
            return Err(SelectionError::DuplicateIdentity(
                candidate.identity.clone(),
            ));
        }
    }
    for name in preferred {
        let mut matches: Vec<_> = candidates
            .iter()
            .filter(|candidate| candidate.class_name.eq_ignore_ascii_case(name))
            .collect();
        matches.sort_by(|left, right| left.identity.cmp(&right.identity));
        match matches.as_slice() {
            [] => {}
            [candidate] => {
                return preferred_choice(candidate, &eligible, &mut probe).map(Some);
            }
            _ => {
                return Err(SelectionError::AmbiguousPreferred {
                    class_name: (*name).to_owned(),
                    identities: matches
                        .iter()
                        .map(|candidate| candidate.identity.clone())
                        .collect(),
                });
            }
        }
    }
    fallback(candidates, &eligible, &mut probe)
}

fn preferred_choice<I: Clone, C>(
    candidate: &Candidate<I>,
    eligible: &impl Fn(&Candidate<I>) -> bool,
    probe: &mut impl FnMut(&Candidate<I>) -> Result<Option<C>, Box<dyn Error>>,
) -> Result<Choice<I, C>, SelectionError<I>> {
    let capabilities = if eligible(candidate) {
        inspect(candidate, probe)?
    } else {
        None
    };
    capabilities
        .map(|capabilities| Choice {
            candidate: candidate.clone(),
            capabilities,
            reason: SelectionReason::Preferred,
        })
        .ok_or_else(|| SelectionError::IncompatiblePreferred {
            identity: candidate.identity.clone(),
        })
}
fn inspect<I: Clone, C>(
    candidate: &Candidate<I>,
    probe: &mut impl FnMut(&Candidate<I>) -> Result<Option<C>, Box<dyn Error>>,
) -> Result<Option<C>, SelectionError<I>> {
    probe(candidate).map_err(|source| SelectionError::Probe {
        identity: candidate.identity.clone(),
        source,
    })
}

fn fallback<I: Clone + Ord, C>(
    candidates: &[Candidate<I>],
    eligible: &impl Fn(&Candidate<I>) -> bool,
    probe: &mut impl FnMut(&Candidate<I>) -> Result<Option<C>, Box<dyn Error>>,
) -> Result<Option<Choice<I, C>>, SelectionError<I>> {
    let mut alternatives: Vec<_> = candidates
        .iter()
        .filter(|candidate| eligible(candidate))
        .collect();
    alternatives.sort_by(|left, right| left.identity.cmp(&right.identity));
    for candidate in alternatives {
        if let Some(capabilities) = inspect(candidate, probe)? {
            return Ok(Some(Choice {
                candidate: candidate.clone(),
                capabilities,
                reason: SelectionReason::Fallback,
            }));
        }
    }
    Ok(None)
}
