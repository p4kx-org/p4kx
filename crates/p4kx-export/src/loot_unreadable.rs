// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use crate::loot_soc::SocError;
use crate::starmap_xml::XmlError;

/// Why a container or one of its files cannot be read for the loot containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Unreadable {
    /// The Archive has no entry at the index of the container.
    #[error("no such entry")]
    NoSuchEntry,
    /// The entry is no container.
    #[error("cannot be opened as a container")]
    NotAContainer,
    /// An entity file is over the size limit or cannot be read.
    #[error("an entity file cannot be read")]
    EntityFile,
    /// An entity file could be read but is not valid XML.
    #[error("an entity file {0}")]
    EntityXml(#[from] XmlError),
    /// An entity places a container that the Archive lacks.
    #[error("placed by an entity but not in the Archive")]
    MissingFromArchive,
    /// A `.soc` file is over the size limit or cannot be read.
    #[error("a .soc is over the limit or cannot be read")]
    SocFile,
    /// A `.soc` file cannot be counted.
    #[error("{0}")]
    Soc(#[from] SocError),
}

/// A container and why it cannot be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnreadableContainer {
    /// The path of the container, as the Archive has it or as written.
    pub(crate) path: String,
    /// Why it cannot be read.
    pub(crate) reason: Unreadable,
}

impl fmt::Display for UnreadableContainer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_container_is_named_with_its_reason() {
        let unreadable = UnreadableContainer {
            path: "a/b.socpak".to_owned(),
            reason: Unreadable::Soc(SocError::NotChunkFile),
        };
        assert_eq!(
            unreadable.to_string(),
            "a/b.socpak: its .soc is no chunk file"
        );
    }
}
