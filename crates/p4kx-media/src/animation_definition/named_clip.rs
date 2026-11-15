// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{
    SourceAttribute, SourceDeclaration, SourceDeclarationKind, SourceElement,
};
use p4kx_extraction::{NamedCafSelector, SourceUseRole};

use super::chrparams_selectors::{self, AnimationAttributes, Selector};
use super::event_bindings;
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

#[path = "wildcard.rs"]
mod wildcard;

pub(super) use wildcard::Wildcard;

pub(super) struct Filepath<'document> {
    list: SourceElement<'document>,
    list_index: u64,
    directive: Option<Directive<'document>>,
    databases: Option<Databases>,
}

struct Directive<'document> {
    element: SourceElement<'document>,
    attributes: Option<AnimationAttributes<'document>>,
}

impl<'document> Filepath<'document> {
    pub(super) fn for_list(
        projection: &DefinitionProjection<'_, '_>,
        element: SourceElement<'document>,
        parent_is_params_root: bool,
        list_index: u64,
    ) -> Option<Self> {
        let ordered_list = matches!(projection.family, DefinitionFamily::Chrparams)
            && parent_is_params_root
            && chrparams_selectors::named(element, "AnimationList");
        ordered_list.then_some(Self {
            list: element,
            list_index,
            directive: None,
            databases: None,
        })
    }

    pub(super) const fn list_index(&self) -> u64 {
        self.list_index
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
        if !chrparams_selectors::named(element, "Animation")
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
                attributes: Selector::read(element).attributes,
            });
            return Ok(());
        }
        if !declaration.qualified_name().eq_ignore_ascii_case("path") {
            return Ok(());
        }
        let Some(attributes) = Selector::read(element).attributes else {
            return Ok(());
        };
        let path = declaration.value();
        let name = &attributes.name;
        if let Some(selector) = NamedCafSelector::new(name.value(), path) {
            let link = Link {
                name: name.value(),
                name_position: declaration_position(name.position())?,
                path,
                path_position: position,
                directive: self.directive.as_ref(),
                list: self.list,
                list_index: self.list_index,
                selector,
            };
            link.write(projection, &mut self.databases, indices, writer)?;
        } else if wildcard::is_wildcard(name.value(), path) {
            wildcard::write(
                &attributes,
                self.list,
                self.directive.as_ref(),
                projection,
                indices,
                writer,
            )?;
        }
        Ok(())
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

struct Link<'directive, 'document> {
    name: &'document str,
    name_position: AtlPosition,
    path: &'document str,
    path_position: AtlPosition,
    directive: Option<&'directive Directive<'document>>,
    list: SourceElement<'document>,
    list_index: u64,
    selector: NamedCafSelector<'document>,
}

impl Link<'_, '_> {
    fn write(
        &self,
        projection: &DefinitionProjection<'_, '_>,
        databases: &mut Option<Databases>,
        indices: [u64; 2],
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        let selector_origin = |position| {
            origins::position(projection.input, position, SourceUseRole::Selector, writer)
        };
        let name = selector_origin(self.name_position)?;
        let path = selector_origin(self.path_position)?;
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
            kind: links::reference_kind(projection, MediaKind::Animation, writer),
            original: &path,
            selector: &full,
        }
        .resolve(projection, &mut prepared, writer)?;
        if let Some(archive_path) = &archive_path {
            self.write_dba_binding(
                &NamedSelection {
                    authored_path: self.path,
                    archive_path,
                    original: &path,
                    selector: &full,
                },
                databases,
                projection,
                indices,
                writer,
            )?;
            self.observe_event_alias(projection, indices, archive_path, full, writer)?;
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

    fn write_dba_binding(
        &self,
        selection: &NamedSelection<'_>,
        databases: &mut Option<Databases>,
        projection: &DefinitionProjection<'_, '_>,
        indices: [u64; 2],
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        let databases = match databases {
            Some(observed) => observed,
            empty @ None => empty.insert(Databases::read(self.list, projection, writer)?),
        };
        databases.write(selection, projection, indices, writer)
    }

    fn observe_event_alias(
        &self,
        projection: &DefinitionProjection<'_, '_>,
        indices: [u64; 2],
        archive_path: &str,
        proof: OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), Failure> {
        projection
            .context
            .directory
            .index
            .animation_event_bindings
            .alias(
                event_bindings::List {
                    entry: projection.context.entry.index(),
                    element: self.list_index,
                },
                indices,
                archive_path,
                proof,
                writer,
            )?;
        Ok(())
    }

    fn archive_path(&self) -> Option<String> {
        self.selector
            .archive_path(self.directive.and_then(Directive::path))
    }
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
        if !link.selector.uses_scope() {
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
