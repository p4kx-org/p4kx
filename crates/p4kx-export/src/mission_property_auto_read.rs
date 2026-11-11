// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::mission_property_auto::{AutoSpawns, SpawnClosetSearchOptions};
use crate::mission_property_source;
use crate::record_read::ReadError;
use crate::source_fields::{self, FieldCapture, FieldChecks, ReferenceShape};

fn fields<'capture, 'view, 'source>(
    source: &'view StructValue<'source>,
    capture: Option<&'capture dyn FieldCapture<'source>>,
) -> Result<FieldChecks<'capture, 'view, 'source>, ReadError> {
    let fields = FieldChecks::for_context(source, "", capture)?;
    fields.check(&["autoSpawns", "searchOptions"])?;
    Ok(fields)
}

pub(crate) fn read<'source>(
    source: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<AutoSpawns, ReadError> {
    let fields = fields(source, capture)?;
    Ok(AutoSpawns {
        auto_spawns: fields
            .reference_view("autoSpawns")?
            .map(ReferenceShape::into_owned),
        search_options: fields
            .structure("searchOptions")?
            .map(|source| search(source, capture))
            .transpose()?,
    })
}

/// Checks the complete supported source shape without materializing presentation data.
#[cfg(test)]
pub(crate) fn validate(source: &StructValue<'_>) -> Result<(), ReadError> {
    validate_context(source, None)
}

/// Checks the same shape while retaining actual original guard failures.
pub(crate) fn validate_observed<'source>(
    source: &StructValue<'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    validate_context(source, Some(capture))
}

fn validate_context<'source>(
    source: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    let fields = fields(source, capture)?;
    fields.reference_shape("autoSpawns")?;
    if let Some(source) = fields.structure("searchOptions")? {
        let view = SearchView::read(source, capture)?;
        match view.kind {
            SearchKind::Base => {}
            SearchKind::EntityDeclaration => {
                view.entity()?;
            }
            SearchKind::Tags => {
                mission_property_source::tag_shape(
                    view.positive()?,
                    "searchOptions.closetPositiveTags",
                    capture,
                )?;
                mission_property_source::tag_shape(
                    view.negative()?,
                    "searchOptions.closetNegativeTags",
                    capture,
                )?;
            }
        }
    }
    Ok(())
}

fn search<'source>(
    source: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<SpawnClosetSearchOptions, ReadError> {
    let view = SearchView::read(source, capture)?;
    Ok(match view.kind {
        SearchKind::Base => SpawnClosetSearchOptions::Base,
        SearchKind::EntityDeclaration => SpawnClosetSearchOptions::EntityDeclaration {
            entity_declaration: view.entity()?.map(ReferenceShape::into_owned),
        },
        SearchKind::Tags => SpawnClosetSearchOptions::Tags {
            closet_positive_tags: mission_property_source::tags_context(
                view.positive()?,
                "searchOptions.closetPositiveTags",
                capture,
            )?,
            closet_negative_tags: mission_property_source::tags_context(
                view.negative()?,
                "searchOptions.closetNegativeTags",
                capture,
            )?,
        },
    })
}

#[derive(Clone, Copy)]
enum SearchKind {
    Base,
    EntityDeclaration,
    Tags,
}

struct SearchView<'capture, 'view, 'source> {
    fields: FieldChecks<'capture, 'view, 'source>,
    kind: SearchKind,
}

impl<'capture, 'view, 'source> SearchView<'capture, 'view, 'source> {
    fn read(
        source: &'view StructValue<'source>,
        capture: Option<&'capture dyn FieldCapture<'source>>,
    ) -> Result<Self, ReadError> {
        let (kind, known) = match source.type_name() {
            "SpawnClosetSearchOptionsBase" => (SearchKind::Base, &[][..]),
            "SpawnClosetSearchOptions_EntityDeclaration" => {
                (SearchKind::EntityDeclaration, &["entityDeclaration"][..])
            }
            "SpawnClosetSearchOptions_Tags" => (
                SearchKind::Tags,
                &["closetPositiveTags", "closetNegativeTags"][..],
            ),
            _ => {
                return Err(source_fields::unknown_struct(
                    source,
                    "searchOptions",
                    capture,
                ));
            }
        };
        let fields = FieldChecks::for_context(source, "searchOptions", capture)?;
        fields.check(known)?;
        Ok(Self { fields, kind })
    }

    fn entity(&self) -> Result<Option<ReferenceShape<'view, 'source>>, ReadError> {
        self.fields.reference_view("entityDeclaration")
    }

    fn positive(&self) -> Result<Option<&'view StructValue<'source>>, ReadError> {
        self.fields.structure("closetPositiveTags")
    }

    fn negative(&self) -> Result<Option<&'view StructValue<'source>>, ReadError> {
        self.fields.structure("closetNegativeTags")
    }
}

#[cfg(test)]
#[path = "mission_property_shape_tests.rs"]
mod tests;
