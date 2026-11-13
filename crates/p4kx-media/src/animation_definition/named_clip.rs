// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::cryxml::{
    SourceAttribute, SourceDeclaration, SourceDeclarationKind, SourceElement,
};

use super::links::{self, ReferenceSelection};
use super::named_dba::{Databases, NamedSelection};
use super::origins;
use super::payload::{self as wire, DefinitionFamily, DefinitionPayload, NamedCaf};
use super::projection::{DefinitionProjection, Failure};
use crate::audio_control_payload::AtlPosition;
use crate::audio_record_origins as records;
use crate::origin_contributions::OriginContributions;
use crate::xml_source_projection::declaration_position;
use crate::{MediaKind, MediaWriter, Status, Subresource};

#[derive(Default)]
pub(super) struct Filepath<'document> {
    directive: Option<Directive<'document>>,
    databases: Databases,
}

struct Directive<'document> {
    element: SourceElement<'document>,
    attributes: Option<AnimationAttributes<'document>>,
}

impl<'document> Filepath<'document> {
    pub(super) fn for_list(
        projection: &DefinitionProjection<'_, '_>,
        element: SourceElement<'document>,
        parent_name: Option<&str>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Option<Self>, Failure> {
        let ordered_list = matches!(projection.family, DefinitionFamily::Chrparams)
            && element.namespace().is_none()
            && element.name().eq_ignore_ascii_case("AnimationList")
            && parent_name.is_some_and(|name| name.eq_ignore_ascii_case("Params"));
        if !ordered_list {
            return Ok(None);
        }
        Ok(Some(Self {
            directive: None,
            databases: Databases::read(element, projection, writer)?,
        }))
    }

    pub(super) fn observe(
        &mut self,
        projection: &DefinitionProjection<'_, '_>,
        element: SourceElement<'document>,
        declaration: &SourceDeclaration<'document>,
        position: AtlPosition,
        indices: [u64; 2],
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        if element.namespace().is_some()
            || !element.name().eq_ignore_ascii_case("Animation")
            || !matches!(
                declaration.kind(),
                SourceDeclarationKind::Attribute { namespace: None }
            )
        {
            return Ok(());
        }
        if declaration.qualified_name().eq_ignore_ascii_case("name")
            && declaration.value().eq_ignore_ascii_case("#filepath")
        {
            self.directive = Some(Directive {
                element,
                attributes: AnimationAttributes::read(element),
            });
            return Ok(());
        }
        if !declaration.qualified_name().eq_ignore_ascii_case("path") {
            return Ok(());
        }
        let Some(attributes) = AnimationAttributes::read(element) else {
            return Ok(());
        };
        let path = declaration.value();
        let name = &attributes.name;
        if is_concrete_caf(name.value(), path) {
            let link = Link {
                name: name.value(),
                name_position: declaration_position(name.position())?,
                path,
                path_position: position,
                directive: self.directive.as_ref(),
            };
            link.write(projection, &self.databases, indices, writer)?;
        }
        Ok(())
    }
}

struct AnimationAttributes<'document> {
    name: SourceAttribute<'document>,
    path: SourceAttribute<'document>,
}

impl<'document> AnimationAttributes<'document> {
    fn read(element: SourceElement<'document>) -> Option<Self> {
        let mut name = None;
        let mut path = None;
        for attribute in element
            .attributes()
            .filter(|attribute| attribute.namespace().is_none())
        {
            let slot = if attribute.name().eq_ignore_ascii_case("name") {
                &mut name
            } else if attribute.name().eq_ignore_ascii_case("path") {
                &mut path
            } else {
                continue;
            };
            if slot.replace(attribute).is_some() {
                return None;
            }
        }
        Some(Self {
            name: name?,
            path: path?,
        })
    }
}

impl<'document> Directive<'document> {
    fn path(&self) -> Option<&'document str> {
        self.attributes
            .as_ref()
            .map(|attributes| attributes.path.value())
    }

    fn evidence(
        &self,
        projection: &DefinitionProjection<'_, '_>,
        writer: &MediaWriter<'_>,
    ) -> Result<Scope, Failure> {
        let mut whole = writer.capture_origins();
        let position = |attribute: &SourceAttribute<'_>| {
            origins::position(
                projection.input,
                declaration_position(attribute.position())?,
                SourceUseRole::Selector,
                writer,
            )
            .map_err(Failure::from)
        };
        if let Some(attributes) = &self.attributes {
            let path = position(&attributes.path)?;
            whole.include(&position(&attributes.name)?, writer)?;
            whole.include(&path, writer)?;
            return Ok(Scope {
                original: path,
                selection: whole.finish(),
            });
        }
        for attribute in self.element.attributes().filter(|attribute| {
            attribute.namespace().is_none()
                && (attribute.name().eq_ignore_ascii_case("name")
                    || attribute.name().eq_ignore_ascii_case("path"))
        }) {
            whole.include(&position(&attribute)?, writer)?;
        }
        let selection = whole.finish();
        Ok(Scope {
            original: selection.clone(),
            selection,
        })
    }
}

fn is_concrete_caf(name: &str, path: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(['#', '$'])
        && !name.contains(['*', '?'])
        && !path.contains(['*', '?'])
        && path
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("caf"))
}

struct Link<'directive, 'document> {
    name: &'document str,
    name_position: AtlPosition,
    path: &'document str,
    path_position: AtlPosition,
    directive: Option<&'directive Directive<'document>>,
}

impl Link<'_, '_> {
    fn write(
        &self,
        projection: &DefinitionProjection<'_, '_>,
        databases: &Databases,
        indices: [u64; 2],
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        let name = origins::position(
            projection.input,
            self.name_position,
            SourceUseRole::Selector,
            writer,
        )?;
        let path = origins::position(
            projection.input,
            self.path_position,
            SourceUseRole::Selector,
            writer,
        )?;
        let scope = Scope::of(self, projection, writer)?;
        let full = combine([&name, &path, &scope.selection], writer)?;
        let path_selection = combine([&path, &scope.selection], writer)?;
        let archive_path = self.archive_path();
        let mut prepared = origins::record(projection.input, path.clone(), writer)?;
        for (field, proof) in [
            ("/data/payload/name", name),
            ("/data/payload/filepath_scope", scope.original),
            ("/data/payload/archive_path", path_selection),
        ] {
            records::field(&mut prepared, field, proof, writer)?;
        }
        let reference = ReferenceSelection {
            authored_path: self.path,
            archive_path: archive_path.as_deref(),
            kind: Some(MediaKind::Animation),
            original: &path,
            selector: &full,
        }
        .resolve(projection, &mut prepared, writer)?;
        if let Some(archive_path) = &archive_path {
            databases.write(
                &NamedSelection {
                    authored_path: self.path,
                    archive_path,
                    original: &path,
                    selector: &full,
                },
                projection,
                indices,
                writer,
            )?;
        }
        prepared.write(
            writer,
            &Subresource::new(wire::NAMED_CAF, indices.to_vec()),
            Status::Complete,
            &DefinitionPayload::NamedCaf(NamedCaf {
                name: self.name,
                authored_path: self.path,
                filepath_scope: self.directive.and_then(Directive::path),
                archive_path,
                reference,
            }),
            &[],
        )?;
        Ok(())
    }

    fn archive_path(&self) -> Option<String> {
        if is_rooted(self.path) {
            return links::archive_animation_path(self.path);
        }
        let directory = self.directive?.path()?.trim_end_matches(['/', '\\']);
        if directory.is_empty() {
            return None;
        }
        links::archive_animation_path(&format!("{directory}/{}", self.path))
    }
}

fn is_rooted(path: &str) -> bool {
    path.split_once(['/', '\\']).is_some_and(|(first, _)| {
        first.eq_ignore_ascii_case("animations") || first.eq_ignore_ascii_case("data")
    })
}

struct Scope {
    original: OriginContributions,
    selection: OriginContributions,
}

impl Scope {
    fn of(
        link: &Link<'_, '_>,
        projection: &DefinitionProjection<'_, '_>,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, Failure> {
        let empty = || writer.capture_origins().finish();
        let Some(directive) = link.directive else {
            return Ok(Self {
                original: empty(),
                selection: empty(),
            });
        };
        let mut evidence = directive.evidence(projection, writer)?;
        if is_rooted(link.path) {
            evidence.selection = empty();
        }
        Ok(evidence)
    }
}

fn combine<'a>(
    inputs: impl IntoIterator<Item = &'a OriginContributions>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, Failure> {
    let mut capture = writer.capture_origins();
    for input in inputs {
        capture.include(input, writer)?;
    }
    Ok(capture.finish())
}
