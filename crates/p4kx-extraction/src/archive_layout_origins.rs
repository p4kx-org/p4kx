// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, MetadataRange, MetadataSection, MetadataView};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::{
    ArchiveLayoutInfo, FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath, FieldPathError,
    InputScope, SourceEvidenceError, SourceInput, SourceInventory, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

/// Field-addressed original evidence for a cached Archive layout summary.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ArchiveLayoutOrigins {
    /// Immediate origin of every value in the layout summary.
    pub field_origins: FieldOrigins,
    /// The physical owners and actual uses referenced by these fields.
    pub provenance: SourceInventory,
}

/// A cached Archive layout could not be addressed or bound to its observed input.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ArchiveLayoutOriginError {
    /// The shared source registry could not retain original evidence.
    #[error(transparent)]
    Source(#[from] SourceEvidenceError),
    /// An exact output field address could not be represented.
    #[error(transparent)]
    Address(#[from] FieldPathError),
    /// Conflicting field ownership was produced.
    #[error(transparent)]
    Field(#[from] FieldOriginsError),
    /// The cached layout could not be visited in its shared wire form.
    #[error("cannot serialize the cached Archive layout")]
    Wire(#[from] serde_json::Error),
}

pub(crate) fn capture(
    archive: &Archive,
    layout: &ArchiveLayoutInfo,
    address: &str,
) -> Result<ArchiveLayoutOrigins, ArchiveLayoutOriginError> {
    let uses = SourceUses::with_compact_use_refs();
    let (scope, complete) = owner(archive, &uses)?;
    let wire = serde_json::to_value(layout)?;
    let mut fields = FieldOrigins::new();
    let mut path = FieldPath::parse(address)?;
    let capture = LayoutCapture {
        uses: &uses,
        scope: scope.as_ref(),
        complete,
        view: archive.metadata_view(),
    };
    capture.visit(&wire, &mut fields, &mut path, "")?;
    Ok(ArchiveLayoutOrigins {
        field_origins: fields,
        provenance: uses.freeze()?,
    })
}

fn owner(
    archive: &Archive,
    uses: &SourceUses,
) -> Result<(Option<SourceScope>, bool), SourceEvidenceError> {
    if let Some(origin) = archive.container_input_origin() {
        let complete = origin.ancestors().all(|step| step.is_some());
        let scope = uses.observe(SourceInput::Archive(&origin), InputScope::Payload)?;
        return Ok((Some(scope), complete));
    }
    let scope = uses.root_directory_scope(archive)?;
    let complete = scope.is_some();
    Ok((scope, complete))
}

struct LayoutCapture<'a> {
    uses: &'a SourceUses,
    scope: Option<&'a SourceScope>,
    complete: bool,
    view: MetadataView<'a>,
}

impl LayoutCapture<'_> {
    fn visit(
        &self,
        value: &Value,
        fields: &mut FieldOrigins,
        path: &mut FieldPath,
        field: &str,
    ) -> Result<Contributions, ArchiveLayoutOriginError> {
        let input = if let Value::Object(children) = value {
            let mut union = Contributions::empty();
            for (key, child) in children {
                path.push_field(key)?;
                let child_field = if field.is_empty() {
                    key.clone()
                } else {
                    format!("{field}/{key}")
                };
                let child_input = self.visit(child, fields, path, &child_field)?;
                union.uses.extend(child_input.uses);
                union.complete &= child_input.complete;
                path.pop();
            }
            union
        } else {
            self.field(field)?
        };
        fields.insert(path.clone(), input.origin(self.uses))?;
        Ok(input)
    }

    fn field(&self, field: &str) -> Result<Contributions, SourceEvidenceError> {
        if field == "version" {
            return Ok(Contributions::empty());
        }
        let Some(scope) = self.scope else {
            return Ok(Contributions::unknown());
        };
        let mut uses = Vec::new();
        let mut complete = self.complete;
        if field == "sourceLength" || needs_source_length(field) {
            uses.push(scope.at(
                SourceLocation::Metadata {
                    field: "archiveLayout/sourceLength".into(),
                },
                if field == "sourceLength" {
                    SourceUseRole::Payload
                } else {
                    SourceUseRole::Conversion
                },
            )?);
        }
        if field != "sourceLength" {
            let sections = self.sections(field);
            let input = self.header_uses(scope, field, sections)?;
            complete &= !sections.is_empty() && input.len() == sections.len();
            uses.extend(input);
        }
        Ok(Contributions { uses, complete })
    }

    fn header_uses(
        &self,
        scope: &SourceScope,
        field: &str,
        sections: &[MetadataSection],
    ) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        let mut uses = Vec::new();
        for &section in sections {
            if let Ok(Some(region)) = self.view.region(section)
                && let Some(range) = region.available()
            {
                uses.push(scope.at(location(section, range, field), role(field))?);
            }
        }
        Ok(uses)
    }

    fn sections(&self, field: &str) -> &'static [MetadataSection] {
        use MetadataSection::{V2Trailer, Zip64End, Zip64Locator, ZipComment, ZipEnd};
        let is_v2 = self.view.metadata().v2_directory().is_some();
        if is_v2 {
            return &[V2Trailer];
        }
        let zip64 = self
            .view
            .metadata()
            .zip_directory()
            .is_some_and(|zip| zip.zip64().is_some());
        let route = if zip64 {
            &[ZipEnd, Zip64Locator, Zip64End][..]
        } else {
            &[ZipEnd][..]
        };
        let (first, rest) = field.split_once('/').unwrap_or((field, ""));
        if first != "zip" {
            return if field == "declarationState" && zip64 {
                &[ZipEnd, Zip64Locator, Zip64End, ZipComment]
            } else {
                route
            };
        }
        match rest.split('/').next() {
            Some("end") => &[ZipEnd],
            Some("zip64" | "records" | "count") if zip64 => &[Zip64End],
            Some("locator") if zip64 => &[Zip64Locator],
            Some("installation") if zip64 && rest.starts_with("installation/tables") => {
                &[Zip64End, ZipComment]
            }
            Some("installation") if zip64 => &[ZipComment],
            _ => route,
        }
    }
}

fn needs_source_length(field: &str) -> bool {
    matches!(field, "declarationState")
        || field.starts_with("zip/end/comment/")
        || field.starts_with("zip/end/following")
        || field.starts_with("zip/zip64/extension/")
        || field.starts_with("v2/trailer/")
        || field.starts_with("zip/installation/tables/")
}

fn role(field: &str) -> SourceUseRole {
    if matches!(field, "zip" | "v2" | "layout") {
        SourceUseRole::Selector
    } else if needs_source_length(field) {
        SourceUseRole::Conversion
    } else {
        SourceUseRole::Payload
    }
}

fn location(section: MetadataSection, range: MetadataRange, field: &str) -> SourceLocation {
    let namespace = match section {
        MetadataSection::ZipEnd => "archiveZipEnd",
        MetadataSection::Zip64End => "archiveZip64End",
        MetadataSection::Zip64Locator => "archiveZip64Locator",
        MetadataSection::ZipComment => "archiveZipComment",
        MetadataSection::V2Trailer => "archiveV2Trailer",
        _ => "archiveLayout",
    };
    SourceLocation::Subresource {
        namespace: namespace.into(),
        indices: vec![range.offset(), range.offset() + range.len()],
        field: field.into(),
    }
}

struct Contributions {
    uses: Vec<SourceUse>,
    complete: bool,
}

impl Contributions {
    const fn empty() -> Self {
        Self {
            uses: Vec::new(),
            complete: true,
        }
    }

    const fn unknown() -> Self {
        Self {
            uses: Vec::new(),
            complete: false,
        }
    }

    fn origin(&self, uses: &SourceUses) -> FieldOrigin {
        uses.origin(&self.uses, self.complete)
    }
}
