// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::mission_property_auto::{AutoSpawns, SpawnClosetSearchOptions};
use crate::mission_property_source;
use crate::record_read::ReadError;
use crate::source_fields::Fields;
use crate::template_issues::TemplateError;

pub(crate) fn read(source: &StructValue<'_>) -> Result<AutoSpawns, ReadError> {
    mission_property_source::check(source, "", &["autoSpawns", "searchOptions"])?;
    let fields = Fields::new(source, "");
    Ok(AutoSpawns {
        auto_spawns: fields.reference("autoSpawns")?,
        search_options: fields.structure("searchOptions")?.map(search).transpose()?,
    })
}

fn search(source: &StructValue<'_>) -> Result<SpawnClosetSearchOptions, ReadError> {
    let fields = Fields::new(source, "searchOptions");
    Ok(match source.type_name() {
        "SpawnClosetSearchOptionsBase" => {
            mission_property_source::check(source, "searchOptions", &[])?;
            SpawnClosetSearchOptions::Base
        }
        "SpawnClosetSearchOptions_EntityDeclaration" => {
            mission_property_source::check(source, "searchOptions", &["entityDeclaration"])?;
            SpawnClosetSearchOptions::EntityDeclaration {
                entity_declaration: fields.reference("entityDeclaration")?,
            }
        }
        "SpawnClosetSearchOptions_Tags" => {
            mission_property_source::check(
                source,
                "searchOptions",
                &["closetPositiveTags", "closetNegativeTags"],
            )?;
            SpawnClosetSearchOptions::Tags {
                closet_positive_tags: mission_property_source::tags(
                    fields.structure("closetPositiveTags")?,
                    "searchOptions.closetPositiveTags",
                )?,
                closet_negative_tags: mission_property_source::tags(
                    fields.structure("closetNegativeTags")?,
                    "searchOptions.closetNegativeTags",
                )?,
            }
        }
        _ => return Err(TemplateError::unknown("searchOptions", source).into()),
    })
}
