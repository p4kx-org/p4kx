// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldOrigin, PositionedReferenceSources, PositionedSources, PositionedValue, RecordJsonBytes,
    RecordJsonLocation, RecordJsonReferenceMember as Member, RecordJsonSource, SourceEvidenceError,
    SourceUse, SourceUseRole, Value,
};

use super::{Capture, Route};
use crate::{KeyText, LineValue, RecordLink};

/// Separate origins of a Record preview field and its displayed value.
#[derive(Debug)]
#[non_exhaustive]
pub struct RecordFieldProvenance {
    /// Authored property name or the derived array ordinal.
    pub name: FieldOrigin,
    /// Actual value or the rendered Reference path and instance location.
    pub value: FieldOrigin,
    /// The selected translation, missing-translation decision, or visible Reference trailer.
    pub annotation: Option<FieldOrigin>,
    /// Original Reference members when the line displays a Reference.
    pub reference: Option<RecordReferenceProvenance>,
    /// Stored GUID when the Record XML reports an unresolved Reference.
    pub dangling_guid: Option<FieldOrigin>,
}

/// Original inputs of one resolved Record preview Reference.
#[derive(Debug)]
#[non_exhaustive]
pub struct RecordReferenceProvenance {
    /// Target GUID.
    pub guid: FieldOrigin,
    /// Actual target path.
    pub path: FieldOrigin,
    /// Actual target type.
    pub type_name: FieldOrigin,
    /// Location within the target Record when the Reference has one.
    pub instance_path: Option<FieldOrigin>,
}

impl Capture<'_> {
    pub(super) fn line(
        &self,
        parent: &Route,
        at: Option<PositionedValue<'_, '_>>,
        shown: &LineValue,
    ) -> Result<RecordFieldProvenance, SourceEvidenceError> {
        let mut name_route = parent.clone();
        if let Some(slot) = at.filter(|slot| slot.field_name().is_none()) {
            self.binding.append(
                &mut name_route,
                slot.array_header_source().and_then(RecordJsonLocation::of),
            )?;
        } else {
            self.binding.positioned(
                &mut name_route,
                at.and_then(|slot| slot.field_name_sources()).as_ref(),
            )?;
        }
        let (value, annotation, reference) = match shown {
            LineValue::Reference(link) => self.link(parent, at, link)?,
            _ => self.value(parent, at, shown)?,
        };
        Ok(RecordFieldProvenance {
            name: name_route.origin(&self.binding.uses),
            value,
            annotation,
            reference,
            dangling_guid: self.dangling_guid(parent, at)?,
        })
    }

    fn dangling_guid(
        &self,
        parent: &Route,
        at: Option<PositionedValue<'_, '_>>,
    ) -> Result<Option<FieldOrigin>, SourceEvidenceError> {
        let Some(sources) = at.and_then(|slot| slot.dangling_reference_guid_sources()) else {
            return Ok(None);
        };
        let mut route = parent.clone();
        self.binding
            .referenced(&mut route, sources.sources(), sources.complete())?;
        Ok(Some(route.origin(&self.binding.uses)))
    }

    fn value(
        &self,
        parent: &Route,
        at: Option<PositionedValue<'_, '_>>,
        shown: &LineValue,
    ) -> Result<VisibleOrigins, SourceEvidenceError> {
        let mut route = parent.clone();
        let coordinates =
            if let Some(error) = at.and_then(|slot| slot.dangling_reference_error_sources()) {
                self.binding
                    .referenced(&mut route, error.sources(), error.complete())?;
                None
            } else {
                let coordinates = at.map(|slot| slot.value_sources());
                self.binding.positioned(&mut route, coordinates.as_ref())?;
                coordinates
            };
        let value = route.origin(&self.binding.uses);
        let annotation = match shown {
            LineValue::Key { text, .. } if !matches!(text, KeyText::None) => {
                let key = at.and_then(|slot| match slot.value() {
                    Value::Locale(key) => Some(*key),
                    _ => None,
                });
                self.translation(parent, coordinates.as_ref(), key)?
            }
            _ => None,
        };
        Ok((value, annotation, None))
    }

    fn link(
        &self,
        parent: &Route,
        at: Option<PositionedValue<'_, '_>>,
        link: &RecordLink,
    ) -> Result<VisibleOrigins, SourceEvidenceError> {
        let reference = at.and_then(|slot| slot.reference_sources());
        let reference = reference.as_ref();
        let path = self.reference_member(parent, reference, Member::Path)?;
        let guid = self.reference_member(parent, reference, Member::Guid)?;
        let kind = self.reference_member(parent, reference, Member::Type)?;
        let instance = link
            .at()
            .map(|_| self.reference_member(parent, reference, Member::InstancePath))
            .transpose()?;
        let origins = &self.binding.uses;
        let value = instance.as_ref().map_or_else(
            || path.origin(origins),
            |instance| {
                let mut both = path.clone();
                both.inputs.extend_from_slice(&instance.inputs);
                both.complete &= instance.complete;
                both.origin(origins)
            },
        );
        let annotation = if link.tag().is_some() {
            self.tag(self.tag_selection(parent, reference)?, link)?
        } else {
            kind.origin(origins)
        };
        Ok((
            value,
            Some(annotation),
            Some(RecordReferenceProvenance {
                guid: guid.origin(origins),
                path: path.origin(origins),
                type_name: kind.origin(origins),
                instance_path: instance.map(|path| path.origin(origins)),
            }),
        ))
    }

    fn reference_member(
        &self,
        parent: &Route,
        reference: Option<&PositionedReferenceSources>,
        member: Member,
    ) -> Result<Route, SourceEvidenceError> {
        let mut route = parent.clone();
        if let Some(reference) = reference {
            let member = reference.member(member);
            self.binding
                .referenced(&mut route, member.sources(), member.complete())?;
        } else {
            route.complete = false;
        }
        Ok(route)
    }

    fn tag_selection(
        &self,
        parent: &Route,
        reference: Option<&PositionedReferenceSources>,
    ) -> Result<Route, SourceEvidenceError> {
        let mut route = parent.clone();
        if let Some(reference) = reference {
            let guid = reference.member(Member::Guid);
            route.complete &= guid.complete();
            for source in guid.sources() {
                self.binding.append(
                    &mut route,
                    RecordJsonLocation::of_use(source).map(as_selector),
                )?;
            }
        } else {
            route.complete = false;
        }
        Ok(route)
    }

    fn translation(
        &self,
        parent: &Route,
        coordinates: Option<&PositionedSources>,
        authored_key: Option<&str>,
    ) -> Result<Option<FieldOrigin>, SourceEvidenceError> {
        let (mut key, selector) = self.localization_key(parent, coordinates)?;
        if let (Some(locale), Some(selector), Some(authored_key)) =
            (self.locale, selector, authored_key)
        {
            key.complete &= locale.append_lookup_uses(
                authored_key,
                &self.binding.uses,
                &selector,
                &mut key.inputs,
            )?;
        } else {
            key.complete = false;
        }
        Ok(Some(key.origin(&self.binding.uses)))
    }

    fn localization_key(
        &self,
        parent: &Route,
        coordinates: Option<&PositionedSources>,
    ) -> Result<(Route, Option<SourceUse>), SourceEvidenceError> {
        let mut key = parent.clone();
        let mut selector = None;
        let Some(coordinates) = coordinates else {
            key.complete = false;
            return Ok((key, None));
        };
        key.complete &= coordinates.complete();
        for &source in coordinates.sources() {
            if let Some(site) = RecordJsonLocation::of(source).map(as_selector) {
                let bound = self.binding.site(site.location, site.role)?;
                if matches!(
                    source,
                    RecordJsonSource::Bytes {
                        kind: RecordJsonBytes::Text,
                        ..
                    }
                ) {
                    selector = Some(bound.clone());
                }
                key.push(bound);
            } else {
                key.complete = false;
            }
        }
        Ok((key, selector))
    }

    fn tag(&self, mut route: Route, link: &RecordLink) -> Result<FieldOrigin, SourceEvidenceError> {
        if let Some(tags) = &self.tags {
            route.complete &= tags.append(link.guid(), &mut route.inputs)?;
        } else {
            route.complete = false;
        }
        Ok(route.origin(&self.binding.uses))
    }
}

fn as_selector(mut site: RecordJsonLocation) -> RecordJsonLocation {
    if site.role == SourceUseRole::Payload {
        site.role = SourceUseRole::Selector;
    }
    site
}

type VisibleOrigins = (
    FieldOrigin,
    Option<FieldOrigin>,
    Option<RecordReferenceProvenance>,
);
