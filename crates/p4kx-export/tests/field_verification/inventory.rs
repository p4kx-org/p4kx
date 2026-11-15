// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_channels::ChannelKind;
use p4kx_extraction::Archive;
use serde::{Deserialize, Serialize};

use super::error::{Error, Kind};
use super::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FieldKey {
    pub(crate) owner: String,
    pub(crate) field: String,
    pub(crate) array_element: bool,
}

impl FieldKey {
    pub(crate) fn new(owner: &str, field: &str) -> Self {
        Self {
            owner: owner.to_owned(),
            field: field.to_owned(),
            array_element: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Disposition {
    Exported { group: String, path: Path },
    ReferenceOnly { reason: String },
    Metadata { reason: String },
    Unsupported { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Mapping {
    pub(crate) source: FieldKey,
    pub(crate) declared_target: Option<String>,
    pub(crate) disposition: Disposition,
    pub(crate) previously_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BuildIdentity {
    channel: String,
    branch: String,
    version: String,
    change_number: String,
    build_id: String,
}

impl BuildIdentity {
    pub(crate) fn of_archive(archive: &Archive) -> Option<Self> {
        let build = archive.build()?;
        let channel = ChannelKind::of_archive(archive.path()).name()?;
        Some(Self {
            channel: channel.to_owned(),
            branch: build.branch().to_owned(),
            version: build.version().to_owned(),
            change_number: build.change_number().to_owned(),
            build_id: build.build_id().to_owned(),
        })
    }

    fn complete(&self) -> bool {
        ChannelKind::from_name(&self.channel).name() == Some(self.channel.as_str())
            && [
                &self.branch,
                &self.version,
                &self.change_number,
                &self.build_id,
            ]
            .into_iter()
            .all(|part| !part.trim().is_empty())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExpectedAbsence {
    source: FieldKey,
    build: BuildIdentity,
    reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Inventory {
    pub(crate) schema_version: u32,
    pub(crate) roots: Vec<String>,
    pub(crate) fields: Vec<Mapping>,
    /// Reviews a declaration's absence only for the exact named manifest and Channel.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) expected_absences: Vec<ExpectedAbsence>,
}

impl Inventory {
    pub(crate) fn reviewed(&self) -> super::Result<BTreeMap<FieldKey, &Mapping>> {
        if self.schema_version != 1
            || self.roots.is_empty()
            || self.fields.len() > 1_000_000
            || self.roots.len() > 10_000
        {
            return Err(Error::new(Kind::Bounds, "invalid inventory version or size").into());
        }
        let mut fields = BTreeMap::new();
        for mapping in &self.fields {
            if mapping.source.owner.is_empty() || mapping.source.field.is_empty() {
                return Err(Error::new(Kind::Mapping, "empty owning field identity").into());
            }
            match &mapping.disposition {
                Disposition::Exported { group, path } => {
                    if group.is_empty() {
                        return Err(Error::new(Kind::Mapping, "empty catalog group").into());
                    }
                    path.validate()?;
                }
                Disposition::ReferenceOnly { reason }
                | Disposition::Metadata { reason }
                | Disposition::Unsupported { reason }
                    if reason.trim().is_empty() =>
                {
                    return Err(Error::new(
                        Kind::Mapping,
                        "an exclusion requires a reviewed reason",
                    )
                    .into());
                }
                _ => {}
            }
            if fields.insert(mapping.source.clone(), mapping).is_some() {
                return Err(Error::new(Kind::Mapping, "duplicate owning field mapping").into());
            }
        }
        self.review_absences(&fields)?;
        Ok(fields)
    }

    fn review_absences(&self, fields: &BTreeMap<FieldKey, &Mapping>) -> super::Result<()> {
        let mut reviewed = BTreeSet::new();
        for absence in &self.expected_absences {
            if !fields.contains_key(&absence.source)
                || !absence.build.complete()
                || absence.reason.trim().is_empty()
            {
                return Err(Error::new(
                    Kind::Mapping,
                    "expected absence needs a reviewed field, complete Build and reason",
                )
                .into());
            }
            if !reviewed.insert((&absence.source, &absence.build)) {
                return Err(Error::new(Kind::Mapping, "duplicate expected Build absence").into());
            }
        }
        Ok(())
    }

    pub(crate) fn absent_on(&self, build: Option<&BuildIdentity>) -> BTreeSet<&FieldKey> {
        self.expected_absences
            .iter()
            .filter_map(|absence| (build == Some(&absence.build)).then_some(&absence.source))
            .collect()
    }
}
