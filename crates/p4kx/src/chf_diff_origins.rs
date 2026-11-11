// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::character::{ChfDiffLookup, ChfDiffMissing, ChfDiffSite, ChfObservedChange};
use p4kx_extraction::{
    FieldOrigin, FieldOrigins, FieldPath, InputScope, ObservedFileRead, SourceInput,
    SourceInventory, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};
use serde::Serialize;

use crate::chf_names::ObservedNames;

pub(crate) struct DiffOrigins {
    uses: SourceUses,
    before: Side,
    after: Side,
}

struct Side {
    scope: SourceScope,
    complete: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Projection {
    Json,
    Text,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffFieldOrigins {
    pub(crate) field_origins: FieldOrigins,
    pub(crate) provenance: SourceInventory,
    #[serde(skip)]
    dispositions: Vec<Option<FieldOrigin>>,
}

impl DiffFieldOrigins {
    pub(crate) fn disposition(&self, row: usize) -> Option<&FieldOrigin> {
        self.dispositions.get(row).and_then(Option::as_ref)
    }
}

impl DiffOrigins {
    pub(crate) fn new(before: &ObservedFileRead, after: &ObservedFileRead) -> Result<Self> {
        let uses = SourceUses::new();
        let before = Side::observe(before, &uses)?;
        let after = Side::observe(after, &uses)?;
        Ok(Self {
            uses,
            before,
            after,
        })
    }

    pub(crate) const fn uses(&self) -> &SourceUses {
        &self.uses
    }

    pub(crate) fn project(
        &self,
        changes: &[ChfObservedChange],
        names: Option<&ObservedNames>,
        projection: Projection,
    ) -> Result<DiffFieldOrigins> {
        let mut fields = FieldOrigins::new();
        for (path, side) in [("/a", &self.before), ("/b", &self.after)] {
            let source = side.scope.for_scope(InputScope::FileMetadata).at(
                SourceLocation::Metadata {
                    field: "path".to_owned(),
                },
                SourceUseRole::Payload,
            )?;
            fields.insert(
                FieldPath::parse(path)?,
                self.uses.origin(&[source], side.complete),
            )?;
        }
        let comparison = [self.before.comparison()?, self.after.comparison()?];
        let origin = self.uses.origin(&comparison, self.complete());
        fields.insert(FieldPath::parse("/identical")?, origin.clone())?;
        if changes.is_empty() {
            fields.insert(FieldPath::parse("/changes")?, origin)?;
        }
        let mut dispositions = Vec::new();
        for (index, change) in changes.iter().enumerate() {
            if projection == Projection::Text {
                let has_disposition =
                    change.change().old.is_some() != change.change().new.is_some();
                dispositions.push(
                    has_disposition
                        .then(|| self.disposition(change))
                        .transpose()?,
                );
            }
            let data = self.change(change, names)?;
            for (key, origin) in ["path", "raw_path", "old", "new"].into_iter().zip(data) {
                fields.insert(
                    FieldPath::parse(&format!("/changes/{index}/{key}"))?,
                    origin,
                )?;
            }
        }
        Ok(DiffFieldOrigins {
            field_origins: fields,
            provenance: self.uses.freeze()?,
            dispositions,
        })
    }

    fn disposition(&self, change: &ChfObservedChange) -> Result<FieldOrigin> {
        let role = SourceUseRole::Selector;
        let sources = [
            self.before.at(change.before(), role)?,
            self.after.at(change.after(), role)?,
        ];
        Ok(self.uses.origin(&sources, self.complete()))
    }

    fn change(
        &self,
        change: &ChfObservedChange,
        names: Option<&ObservedNames>,
    ) -> Result<[FieldOrigin; 4]> {
        let raw = self.raw_path(change)?;
        let raw_path = self.uses.origin(&raw, self.complete());
        let mut path = raw;
        let mut complete = self.complete();
        for lookup in change.path_lookups() {
            complete &= self.append_name(&mut path, lookup, names)?;
        }
        let old = self.value(
            &self.before,
            change.before(),
            change.change().old.is_some(),
            change.order_lookups(),
            names,
        )?;
        let new = self.value(
            &self.after,
            change.after(),
            change.change().new.is_some(),
            change.order_lookups(),
            names,
        )?;
        Ok([self.uses.origin(&path, complete), raw_path, old, new])
    }

    fn raw_path(&self, change: &ChfObservedChange) -> Result<Vec<SourceUse>> {
        let role = SourceUseRole::Selector;
        let mut sources = vec![
            self.before.at(change.before(), role)?,
            self.after.at(change.after(), role)?,
        ];
        for lookup in change.path_lookups() {
            for (side, site) in [
                (&self.before, lookup.before()),
                (&self.after, lookup.after()),
            ] {
                if let Some(site) = site {
                    sources.push(side.at(site, role)?);
                }
            }
        }
        Ok(sources)
    }

    fn value(
        &self,
        input: &Side,
        site: &ChfDiffSite,
        present: bool,
        lookups: &[ChfDiffLookup],
        names: Option<&ObservedNames>,
    ) -> Result<FieldOrigin> {
        let role = if !present {
            SourceUseRole::Selector
        } else if lookups.is_empty() {
            SourceUseRole::Payload
        } else {
            SourceUseRole::Conversion
        };
        let mut sources = vec![input.at(site, role)?];
        let mut complete = input.complete;
        for lookup in lookups {
            for (operand, site) in [
                (&self.before, lookup.before()),
                (&self.after, lookup.after()),
            ] {
                if let Some(site) = site {
                    sources.push(operand.at(site, SourceUseRole::Selector)?);
                    complete &= operand.complete;
                }
            }
            complete &= self.append_name(&mut sources, lookup, names)?;
        }
        Ok(self.uses.origin(&sources, complete))
    }

    fn append_name(
        &self,
        sources: &mut Vec<SourceUse>,
        lookup: &ChfDiffLookup,
        names: Option<&ObservedNames>,
    ) -> Result<bool> {
        if let (Some(names), Some(kind)) = (names, lookup.kind())
            && let Some(name) = names.lookup(
                kind,
                lookup.hash(),
                &[&self.before.scope, &self.after.scope],
            )?
        {
            sources.extend(name.uses);
            return Ok(name.complete && self.complete());
        }
        Ok(true)
    }

    const fn complete(&self) -> bool {
        self.before.complete && self.after.complete
    }
}

impl Side {
    fn observe(input: &ObservedFileRead, uses: &SourceUses) -> Result<Self> {
        let source = input
            .origin()
            .map_or(SourceInput::Unknown, SourceInput::File);
        let scope = uses.observe(source, InputScope::Payload)?;
        Ok(Self {
            scope,
            complete: input.origin().is_some(),
        })
    }

    fn at(&self, site: &ChfDiffSite, role: SourceUseRole) -> Result<SourceUse> {
        let (namespace, indices) = match site.missing() {
            Some(ChfDiffMissing::Index(index)) => ("chfDiffMissingIndex", vec![index as u64]),
            Some(ChfDiffMissing::Hash { hash, ordinal }) => {
                ("chfDiffMissingHash", vec![u64::from(hash), ordinal as u64])
            }
            Some(_) => {
                return Err(anyhow::anyhow!(
                    "unknown CHF comparison membership selector"
                ));
            }
            None => ("chf", Vec::new()),
        };
        Ok(self.scope.at(
            SourceLocation::Subresource {
                namespace: namespace.to_owned(),
                indices,
                field: site.pointer().to_owned(),
            },
            role,
        )?)
    }

    fn comparison(&self) -> Result<SourceUse> {
        Ok(self.scope.at(
            SourceLocation::Subresource {
                namespace: "chfDiff".to_owned(),
                indices: Vec::new(),
                field: "comparedCharacterAndHeaderFlags".to_owned(),
            },
            SourceUseRole::Conversion,
        )?)
    }
}
