// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

use crate::plan::{path_key, under};
use crate::{EntityLookupError, Incompatibility, LoadoutError, LoadoutPicks};

/// Own configuration of a Loadout: branches to leave out, Entities to attach instead and the
/// alternatives of generated Loadouts to take ([`LoadoutPicks`]).
///
/// A port is named by its path of port names from the root Entity, joined by `/` and matched
/// ignoring ASCII case, the path [`EntityNode::walk`](crate::EntityNode::walk) lists, e.g.
/// `hardpoint_weapon_nose/hardpoint_class_2`. Apply the changes with
/// [`LoadoutResolver::resolve_with`](crate::LoadoutResolver::resolve_with).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadoutChanges {
    exclusions: Vec<String>,
    replacements: Vec<(String, String)>,
    picks: LoadoutPicks,
}

impl LoadoutChanges {
    /// Creates changes that leave the Loadout as it is.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Leaves out what the port `port` holds, with everything attached below it.
    #[must_use]
    pub fn exclude(mut self, port: impl Into<String>) -> Self {
        self.exclusions.push(port.into());
        self
    }

    /// Attaches the Entity `entity` to the port `port` instead of what it holds. `entity` is a
    /// class name, a GUID or a record path, as [`Entities::find`](crate::Entities::find) takes
    /// it. The Entity brings its own Loadout; the Loadout of the Entity it replaces is dropped.
    #[must_use]
    pub fn replace(mut self, port: impl Into<String>, entity: impl Into<String>) -> Self {
        self.replacements.push((port.into(), entity.into()));
        self
    }

    /// Takes the alternatives `picks` names from the groups of generated Loadouts.
    #[must_use]
    pub fn with_picks(mut self, picks: LoadoutPicks) -> Self {
        self.picks = picks;
        self
    }

    /// Returns the picks of the alternatives of generated Loadouts.
    #[must_use]
    pub const fn picks(&self) -> &LoadoutPicks {
        &self.picks
    }

    /// Replaces the picks of the alternatives of generated Loadouts, in place.
    pub fn set_picks(&mut self, picks: LoadoutPicks) {
        self.picks = picks;
    }

    /// Splits `text` of the form `<port>=<entity>` and adds it as a replacement.
    ///
    /// # Errors
    ///
    /// Returns [`ChangeError::InvalidReplacement`] when `text` lacks the `=` or names no port or
    /// no Entity.
    pub fn replace_text(self, text: &str) -> Result<Self, ChangeError> {
        let (port, entity) = split_replacement(text)?;
        Ok(self.replace(port, entity))
    }

    /// Leaves out what the port `port` holds, in place. Whatever the changes already say about
    /// the port and the ports below it is dropped, as the exclusion covers it.
    ///
    /// # Errors
    ///
    /// Returns [`ChangeError::InvalidPort`] when `port` is no port path.
    pub fn set_excluded(&mut self, port: &str) -> Result<(), ChangeError> {
        let key = checked_key(port)?;
        self.drop_below(&key);
        self.drop_at(&key);
        self.exclusions.push(port.trim().to_owned());
        Ok(())
    }

    /// Attaches the Entity `entity` to the port `port` instead of what it holds, in place. The
    /// change replaces what the changes said about the port; changes to the ports below it are
    /// dropped, as the Entity brings its own Loadout.
    ///
    /// # Errors
    ///
    /// Returns [`ChangeError::InvalidPort`] when `port` is no port path and
    /// [`ChangeError::Conflict`] when a port above it is left out; the changes stay as they were.
    pub fn set_replacement(
        &mut self,
        port: &str,
        entity: impl Into<String>,
    ) -> Result<(), ChangeError> {
        let key = checked_key(port)?;
        if let Some(excluded) = self
            .exclusions
            .iter()
            .find(|path| path_key(path).is_some_and(|ancestor| under(&key, &ancestor)))
        {
            return Err(ChangeError::Conflict {
                path: port.trim().to_owned(),
                other: excluded.clone(),
            });
        }
        self.drop_below(&key);
        self.drop_at(&key);
        self.replacements
            .push((port.trim().to_owned(), entity.into()));
        Ok(())
    }

    /// Removes the change to the port `port`, an exclusion or a replacement, and returns whether
    /// there was one. Taking back a replacement also drops the changes to the ports below it, as
    /// they name ports of the replacing Entity; those below an exclusion are dropped already.
    pub fn remove(&mut self, port: &str) -> bool {
        let Some(key) = path_key(port) else {
            return false;
        };
        let replaced = matches!(self.action_for(port), PortChange::Replace(_));
        let removed = self.drop_at(&key);
        if removed && replaced {
            self.drop_below(&key);
        }
        removed
    }

    /// Returns what the changes do to the port `port`; a path that is none or has no change is
    /// [`PortChange::Keep`].
    #[must_use]
    pub fn action_for(&self, port: &str) -> PortChange<'_> {
        let Some(key) = path_key(port) else {
            return PortChange::Keep;
        };
        if self
            .exclusions
            .iter()
            .any(|path| path_key(path).as_deref() == Some(key.as_str()))
        {
            return PortChange::Exclude;
        }
        self.replacements
            .iter()
            .find(|(path, _)| path_key(path).as_deref() == Some(key.as_str()))
            .map_or(PortChange::Keep, |(_, entity)| PortChange::Replace(entity))
    }

    fn drop_at(&mut self, key: &str) -> bool {
        let before = self.exclusions.len() + self.replacements.len();
        self.exclusions
            .retain(|path| path_key(path).as_deref() != Some(key));
        self.replacements
            .retain(|(path, _)| path_key(path).as_deref() != Some(key));
        self.exclusions.len() + self.replacements.len() != before
    }

    fn drop_below(&mut self, key: &str) {
        self.exclusions
            .retain(|path| !path_key(path).is_some_and(|other| under(&other, key)));
        self.replacements
            .retain(|(path, _)| !path_key(path).is_some_and(|other| under(&other, key)));
    }

    /// Returns whether the changes leave the Loadout as it is.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.has_no_port_changes() && self.picks.is_empty()
    }

    /// Returns whether no port is excluded or replaced.
    pub(crate) const fn has_no_port_changes(&self) -> bool {
        self.exclusions.is_empty() && self.replacements.is_empty()
    }

    /// Returns the ports to leave out, as given.
    #[must_use]
    pub fn exclusions(&self) -> &[String] {
        &self.exclusions
    }

    /// Returns the replacements as `(port, entity)`, as given.
    #[must_use]
    pub fn replacements(&self) -> &[(String, String)] {
        &self.replacements
    }
}

/// A Loadout tree with the changes applied, and the changes that could not be.
#[derive(Debug, Clone)]
pub struct ChangedLoadout {
    pub(crate) node: crate::EntityNode,
    pub(crate) ignored: Vec<String>,
    pub(crate) mismatches: Vec<ReplacementMismatch>,
}

/// A replacement that was applied although its Entity does not fit the port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementMismatch {
    pub(crate) path: String,
    pub(crate) entity: String,
    pub(crate) reason: Incompatibility,
}

impl ReplacementMismatch {
    /// Returns the path of the port, as given.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the class name of the Entity.
    #[must_use]
    pub fn entity(&self) -> &str {
        &self.entity
    }

    /// Returns what does not fit.
    #[must_use]
    pub const fn reason(&self) -> &Incompatibility {
        &self.reason
    }
}

impl ChangedLoadout {
    /// Returns the replacements that were applied although the Entity does not fit the port's
    /// types and sizes.
    #[must_use]
    pub fn mismatches(&self) -> &[ReplacementMismatch] {
        &self.mismatches
    }

    /// Returns the tree.
    #[must_use]
    pub const fn node(&self) -> &crate::EntityNode {
        &self.node
    }

    /// Returns the tree, dropping the ignored changes.
    #[must_use]
    pub fn into_node(self) -> crate::EntityNode {
        self.node
    }

    /// Returns the paths, as given, of the changes to ports below a docked vehicle that is not
    /// expanded: the tree has no such port, so they were not applied and are no error.
    #[must_use]
    pub fn ignored(&self) -> &[String] {
        &self.ignored
    }
}

/// What [`LoadoutChanges`] do to one port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortChange<'c> {
    /// The port keeps what the Loadout gives it.
    Keep,
    /// The port is left out.
    Exclude,
    /// The Entity, as the replacement names it, is attached instead.
    Replace(&'c str),
}

fn checked_key(path: &str) -> Result<String, ChangeError> {
    path_key(path).ok_or_else(|| ChangeError::InvalidPort {
        path: path.to_owned(),
    })
}

fn split_replacement(text: &str) -> Result<(&str, &str), ChangeError> {
    text.split_once('=')
        .map(|(port, entity)| (port.trim(), entity.trim()))
        .filter(|(port, entity)| !port.is_empty() && !entity.is_empty())
        .ok_or_else(|| ChangeError::InvalidReplacement {
            text: text.to_owned(),
        })
}

/// Why changes to a Loadout cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ChangeError {
    /// A replacement is not of the form `<port>=<entity>`.
    #[error("{text:?} is not of the form <port>=<entity>")]
    InvalidReplacement {
        /// The text as given.
        text: String,
    },
    /// A pick is neither `<group>=<index>` nor an index.
    #[error("{text:?} is not of the form <group>=<index> or an index")]
    InvalidPick {
        /// The text as given.
        text: String,
    },
    /// A port path is empty or has an empty segment.
    #[error("{path:?} is no port path: port names joined by '/'")]
    InvalidPort {
        /// The path as given.
        path: String,
    },
    /// The Loadout has no port of this path.
    #[error("the Loadout has no port {path}")]
    UnknownPort {
        /// The path as given.
        path: String,
    },
    /// Two changes contradict each other: one port is excluded and replaced, or a replacement
    /// lies in an excluded branch.
    #[error("the changes to {path} and {other} contradict each other")]
    Conflict {
        /// The path of the replacement.
        path: String,
        /// The path of the exclusion.
        other: String,
    },
    /// A port is replaced twice.
    #[error("the port {path} is replaced more than once")]
    Duplicate {
        /// The path as given.
        path: String,
    },
    /// The Entity a replacement names cannot be found.
    #[error("the replacement for {path}: {source}")]
    Entity {
        /// The path of the port.
        path: String,
        /// Why the Entity is not found.
        source: EntityLookupError,
    },
    /// The Entity a replacement names cannot be read.
    #[error("the replacement {entity} for {path} cannot be used: {source}")]
    Unreadable {
        /// The path of the port.
        path: String,
        /// The Entity as given.
        entity: String,
        /// Why it cannot be read.
        source: LoadoutError,
    },
    /// The replacement is the root Entity or an Entity the port lies below, so the tree would
    /// contain itself.
    #[error("{entity} cannot replace the port {path}: the port lies below it")]
    Cycle {
        /// The path of the port.
        path: String,
        /// The class name of the Entity.
        entity: String,
    },
    /// The Entity whose Loadout is resolved cannot be.
    #[error(transparent)]
    Loadout(#[from] LoadoutError),
    /// The resolver was stopped before the Loadout was resolved.
    #[error("resolving the Loadout was stopped")]
    Cancelled,
}
