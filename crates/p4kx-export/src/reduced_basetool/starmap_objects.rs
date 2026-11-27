// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::{Serialize, Serializer};

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::number::Number;
use super::record_identity::RecordIdentity;
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use super::selected_texts::{SelectedTexts, TextRefs};
use super::starmap_definitions::DefinitionFamily;
use crate::catalog_queries::starmap::{
    StarmapDefinitionFamily, StarmapDefinitionRows, StarmapDefinitionSelection,
    StarmapDefinitionSnapshot, StarmapObjectDefinition, StarmapObjectLinks, StarmapQuantumTravel,
    VisibilityFields,
};
use crate::catalog_queries::{CatalogRecordSelection, GameTextRef, ParsedCatalogQueries};
use crate::catalog_query_session;
use crate::catalog_rows::staged::StagedCatalogRows;
use crate::resource_float::ResourceFloat64;
use crate::resource_number::ResourceNumber;
use crate::source_value_catalog::SourceRecordReference;

pub(super) const PATH: &str = "definitions.starmapObjects";

pub(super) struct Dependencies<'staged> {
    pub(super) types: &'staged RecordTargets,
    pub(super) amenities: &'staged RecordTargets,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    dependencies: &Dependencies<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<DefinitionFamily, ReducedError> {
    let selection = StarmapDefinitionSelection::new(
        StarmapDefinitionFamily::Objects,
        CatalogRecordSelection::All,
    );
    parsed.with_starmap_definitions(selection, cancel, |snapshot| {
        stage(&snapshot, dependencies, scratch, cancel, texts)
    })?
}

fn stage(
    snapshot: &StarmapDefinitionSnapshot<'_>,
    dependencies: &Dependencies<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<DefinitionFamily, ReducedError> {
    let StarmapDefinitionRows::Objects(rows) = &snapshot.rows else {
        return Err(ReducedError::IncompleteScope(PATH));
    };
    completeness::inventory(PATH, snapshot.availability, &snapshot.coverage)?;
    let mut targets = RecordTargets::default();
    for source in rows {
        catalog_query_session::checkpoint(cancel)?;
        let header = &source.source;
        completeness::value(PATH, &header.guid())?;
        completeness::value(PATH, header.record_index())?;
        completeness::value(PATH, header.class_name())?;
        completeness::value(PATH, header.record_path())?;
        targets.insert(PATH, header)?;
    }

    let mut references = TextRefs::default();
    let mut output = StagedCatalogRows::new(scratch, cancel);
    for source in rows {
        output.append(&Object::read(
            source,
            dependencies,
            &targets,
            &mut references,
        )?)?;
    }
    let table = output.finish(snapshot.availability)?;
    completeness::rows(PATH, rows.len(), table.len())?;
    texts.add(
        references,
        &snapshot.public_names,
        &snapshot.game_texts,
        cancel,
    )?;
    Ok(DefinitionFamily { table, targets })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Object<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    name: Cell<&'read GameTextRef>,
    description: Cell<&'read GameTextRef>,
    size: Cell<DisplaySize<'read>>,
    #[serde(flatten)]
    links: Links<'read>,
    #[serde(flatten)]
    visibility: Visibility<'read>,
    quantum_travel_data: Cell<TravelRadii>,
}

impl<'read> Object<'read> {
    fn read(
        source: &'read StarmapObjectDefinition<'_>,
        dependencies: &Dependencies<'_>,
        objects: &RecordTargets,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let name = completeness::cell(PATH, &source.text.name)?;
        let description = completeness::cell(PATH, &source.display.description)?;
        references.game_text(name.value);
        references.game_text(description.value);
        Ok(Self {
            identity: (&source.source).into(),
            name,
            description,
            size: completeness::cell(PATH, &source.display.size)?.map(DisplaySize),
            links: Links::read(&source.links, dependencies, objects)?,
            visibility: Visibility::read(&source.visibility)?,
            quantum_travel_data: completeness::cell(PATH, &source.owned.quantum_travel_data)?
                .try_map(TravelRadii::read)?,
        })
    }
}

struct DisplaySize<'read>(&'read ResourceFloat64);

impl Serialize for DisplaySize<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Number::from(&ResourceNumber::Double(self.0.clone())).serialize(serializer)
    }
}

type Link<'read> = Cell<RecordLink<'read>>;
type SourceLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Links<'read> {
    r#type: Link<'read>,
    parent: Link<'read>,
    amenities: Cell<Vec<Link<'read>>>,
    jurisdiction: Link<'read>,
    affiliation: Link<'read>,
}

impl<'read> Links<'read> {
    fn read(
        source: &'read StarmapObjectLinks<'_>,
        dependencies: &Dependencies<'_>,
        objects: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            r#type: linked(&source.r#type, dependencies.types)?,
            parent: linked(&source.parent, objects)?,
            amenities: completeness::list(PATH, &source.amenities, |link| {
                RecordLink::read_source(PATH, link, dependencies.amenities, |_| true)
            })?,
            jurisdiction: reference(&source.jurisdiction)?,
            affiliation: reference(&source.affiliation)?,
        })
    }
}

fn linked<'read>(
    source: &'read SourceLink<'_>,
    targets: &RecordTargets,
) -> Result<Link<'read>, ReducedError> {
    completeness::cell(PATH, source)?
        .try_map(|link| RecordLink::read_source(PATH, link, targets, |_| true))
}

fn reference<'read>(source: &'read SourceLink<'_>) -> Result<Link<'read>, ReducedError> {
    completeness::cell(PATH, source)?.try_map(|link| RecordLink::from_source(PATH, link))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Visibility<'read> {
    hide_in_starmap: Cell<&'read bool>,
    hide_in_world: Cell<&'read bool>,
    hide_when_in_adoption_radius: Cell<&'read bool>,
    only_show_when_parent_selected: Cell<&'read bool>,
    block_travel: Cell<&'read bool>,
}

impl<'read> Visibility<'read> {
    fn read(source: &'read VisibilityFields) -> Result<Self, ReducedError> {
        Ok(Self {
            hide_in_starmap: completeness::cell(PATH, &source.hide_in_starmap)?,
            hide_in_world: completeness::cell(PATH, &source.hide_in_world)?,
            hide_when_in_adoption_radius: completeness::cell(
                PATH,
                &source.hide_when_in_adoption_radius,
            )?,
            only_show_when_parent_selected: completeness::cell(
                PATH,
                &source.only_show_when_parent_selected,
            )?,
            block_travel: completeness::cell(PATH, &source.block_travel)?,
        })
    }
}

#[derive(Serialize)]
struct TravelRadii {
    #[serde(rename = "obstructionRadius")]
    obstruction: Cell<Number<'static>>,
    #[serde(rename = "arrivalRadius")]
    arrival: Cell<Number<'static>>,
    #[serde(rename = "adoptionRadius")]
    adoption: Cell<Number<'static>>,
}

impl TravelRadii {
    fn read(source: &StarmapQuantumTravel<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            obstruction: completeness::cell(PATH, &source.obstruction_radius)?.map(Number::from),
            arrival: completeness::cell(PATH, &source.arrival_radius)?.map(Number::from),
            adoption: completeness::cell(PATH, &source.adoption_radius)?.map(Number::from),
        })
    }
}
