// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::cryxml::{
    SourceAttribute, SourceDeclaration, SourceDeclarationKind, SourceElement,
};

use super::links::{self, ReferenceSelection};
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
}

struct Directive<'document> {
    path: &'document str,
    name_position: AtlPosition,
    path_position: AtlPosition,
}

impl<'document> Filepath<'document> {
    pub(super) fn for_list(
        family: DefinitionFamily,
        element: SourceElement<'document>,
        parent_name: Option<&str>,
    ) -> Option<Self> {
        (matches!(family, DefinitionFamily::Chrparams)
            && element.namespace().is_none()
            && element.name().eq_ignore_ascii_case("AnimationList")
            && parent_name.is_some_and(|name| name.eq_ignore_ascii_case("Params")))
        .then(Self::default)
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
            || !declaration.qualified_name().eq_ignore_ascii_case("path")
            || !matches!(
                declaration.kind(),
                SourceDeclarationKind::Attribute { namespace: None }
            )
        {
            return Ok(());
        }
        let Some(name) = unique_animation_name(element) else {
            return Ok(());
        };
        let path = declaration.value();
        if name.value().eq_ignore_ascii_case("#filepath") {
            self.directive = Some(Directive {
                path,
                name_position: declaration_position(name.position())?,
                path_position: position,
            });
        } else if is_concrete_caf(name.value(), path) {
            let link = Link {
                name: name.value(),
                name_position: declaration_position(name.position())?,
                path,
                path_position: position,
                directive: self.directive.as_ref(),
            };
            link.write(projection, indices, writer)?;
        }
        Ok(())
    }
}

fn unique_animation_name(element: SourceElement<'_>) -> Option<SourceAttribute<'_>> {
    let mut name = None;
    let mut seen_path = false;
    for attribute in element
        .attributes()
        .filter(|attribute| attribute.namespace().is_none())
    {
        if attribute.name().eq_ignore_ascii_case("name") {
            if name.replace(attribute).is_some() {
                return None;
            }
        } else if attribute.name().eq_ignore_ascii_case("path") {
            if seen_path {
                return None;
            }
            seen_path = true;
        }
    }
    name
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
            kind: MediaKind::Animation,
            original: &path,
            selector: &full,
        }
        .resolve(projection, &mut prepared, writer)?;
        prepared.write(
            writer,
            &Subresource::new(wire::NAMED_CAF, indices.to_vec()),
            Status::Complete,
            &DefinitionPayload::NamedCaf(NamedCaf {
                name: self.name,
                authored_path: self.path,
                filepath_scope: self.directive.map(|directive| directive.path),
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
        let directory = self.directive?.path.trim_end_matches(['/', '\\']);
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
        let mut scope = writer.capture_origins();
        let mut original = writer.capture_origins();
        if let Some(directive) = link.directive {
            let path = origins::position(
                projection.input,
                directive.path_position,
                SourceUseRole::Selector,
                writer,
            )?;
            original.include(&path, writer)?;
            if !is_rooted(link.path) {
                let name = origins::position(
                    projection.input,
                    directive.name_position,
                    SourceUseRole::Selector,
                    writer,
                )?;
                scope.include(&name, writer)?;
                scope.include(&path, writer)?;
            }
        }
        Ok(Self {
            original: original.finish(),
            selection: scope.finish(),
        })
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
