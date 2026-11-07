// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    ArchiveProvider, MediaError, MediaKind, MediaWriter, RootSource, ShaderProvider, SourceContext,
};

/// Whether a registered provider can execute in this build.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum Availability {
    /// The implementation can execute.
    Available,
    /// Its required implementation or dependency is unavailable.
    Unavailable {
        /// Technical reason for refusing the requested kind.
        reason: String,
    },
}

/// Typed projection coverage for one advertised source field group.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct FieldGroup {
    /// Stable source-field group name.
    pub name: String,
    /// Whether typed fields in this group are projected.
    pub projected: bool,
    /// Precise supported scope or reason the group is not projected.
    pub scope: String,
}

/// Explicit provider version, availability and preservation scope.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct Capability {
    /// Output kind.
    pub kind: MediaKind,
    /// Stable implementation identifier.
    pub id: String,
    /// Provider's semantic version.
    pub version: String,
    /// Runtime availability.
    pub availability: Availability,
    /// Exact source-preservation boundary, independent of typed projection.
    pub preservation: String,
    /// Advertised typed field groups and unsupported groups.
    pub field_groups: Vec<FieldGroup>,
}

impl Capability {
    /// Declares an available provider with explicit identity, version and preservation scope.
    #[must_use]
    pub fn new(
        kind: MediaKind,
        id: impl Into<String>,
        version: impl Into<String>,
        preservation: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            id: id.into(),
            version: version.into(),
            availability: Availability::Available,
            preservation: preservation.into(),
            field_groups: Vec::new(),
        }
    }
}

impl FieldGroup {
    /// Declares the exact projected scope of one field group.
    #[must_use]
    pub fn new(name: impl Into<String>, scope: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            projected: true,
            scope: scope.into(),
        }
    }

    /// Declares a field group whose typed projection is unavailable.
    #[must_use]
    pub fn new_with_unsupported_projection(
        name: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            projected: false,
            scope: reason.into(),
        }
    }
}

/// Applicability result for one physical Entry callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Projection {
    /// At least one metadata record was written.
    Written,
    /// This source does not belong to this provider.
    Skipped,
}

/// Projects one retained physical source at a time without owning publication.
/// Providers bound working memory and poll the writer between units of domain work.
pub trait Provider: Send + Sync {
    /// Returns this build's honest capability declaration.
    fn capability(&self) -> Capability;

    /// Projects optional root metadata.
    ///
    /// # Errors
    /// Returns a fatal projection or writer error.
    fn root(&self, _root: &RootSource, _writer: &mut MediaWriter<'_>) -> Result<(), MediaError> {
        Ok(())
    }

    /// Streams records for one exact retained source.
    ///
    /// # Errors
    /// Returns a fatal projection or writer error; no complete generation is published.
    fn source(
        &self,
        source: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError>;
}

/// Ordered provider registration, replaceable without changing the writer.
pub struct ProviderRegistry {
    pub(crate) providers: Vec<Box<dyn Provider>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self {
            providers: vec![
                Box::new(ArchiveProvider),
                Box::new(ShaderProvider::default()),
            ],
        }
    }
}

impl ProviderRegistry {
    /// Constructs a registry with unique kinds in caller-defined order.
    ///
    /// # Errors
    /// Returns a contract error for duplicate kinds or invalid capability declarations.
    pub fn try_new(providers: Vec<Box<dyn Provider>>) -> Result<Self, MediaError> {
        let mut kinds = Vec::new();
        for provider in &providers {
            let capability = provider.capability();
            if kinds.contains(&capability.kind)
                || !crate::media_record::namespaced(&capability.id)
                || semver::Version::parse(&capability.version).is_err()
                || capability.preservation.trim().is_empty()
            {
                return Err(MediaError::Contract(
                    "duplicate kind or invalid provider identity, version or preservation scope"
                        .into(),
                ));
            }
            let mut groups = std::collections::HashSet::new();
            for group in &capability.field_groups {
                if group.name.trim().is_empty()
                    || group.scope.trim().is_empty()
                    || !groups.insert(&group.name)
                {
                    return Err(MediaError::Contract(
                        "invalid or duplicate projected field group".into(),
                    ));
                }
            }
            kinds.push(capability.kind);
        }
        Ok(Self { providers })
    }

    pub(crate) fn selected(&self, kinds: &[MediaKind]) -> Result<Vec<&dyn Provider>, MediaError> {
        for kind in kinds {
            if !self
                .providers
                .iter()
                .any(|provider| provider.capability().kind == *kind)
            {
                return Err(MediaError::Unsupported {
                    kind: *kind,
                    detail: "provider is not registered".into(),
                });
            }
        }
        let mut selected = Vec::new();
        for provider in &self.providers {
            let capability = provider.capability();
            if !kinds.is_empty() && !kinds.contains(&capability.kind) {
                continue;
            }
            if let Availability::Unavailable { reason } = capability.availability {
                return Err(MediaError::Unsupported {
                    kind: capability.kind,
                    detail: reason,
                });
            }
            selected.push(provider.as_ref());
        }
        if selected.is_empty() {
            return Err(MediaError::Contract("no media providers registered".into()));
        }
        Ok(selected)
    }
}
