// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::equipment::planet_audio::{self, ProceduralPlanetAudioSnapshot};
use p4kx_export::catalog_queries::planet_resources::{self, PlanetResourceSelection};
use p4kx_export::catalog_queries::soils::{self, LooseDefinitionSelection};
use p4kx_export::catalog_queries::starmap::{
    self, StarmapDefinitionFamily, StarmapDefinitionRows, StarmapDefinitionSelection,
    StarmapDefinitionSnapshot,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;

use crate::real_archive_catalog_table_parity::{TestResult, with_bound_measurement_from_producer};
use crate::real_archive_scoped_equipment_sdk::equipment_input;
use crate::real_archive_verification::print_line;

#[path = "real_archive_world_definition_evidence.rs"]
mod evidence;
use evidence::{Expected, Proof, RecordScope};

const COUNT: usize = 8;
const EXPECTED_ROWS: &str = "P4KX_WORLD_DEFINITION_SDK_EXPECTED_ROWS";
const OBJECT_LEFTOVERS: &str = "P4KX_WORLD_DEFINITION_SDK_OBJECT_LEFTOVERS";
const COLD_DATASET: &str = "P4KX_WORLD_DEFINITION_SDK_COLD_DATASET";
const PRODUCER: &str = "P4KX_WORLD_DEFINITION_CATALOG_PRODUCER_REVISION";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Family {
    StarmapObjects,
    StarmapTypes,
    StarmapPresets,
    StarmapAmenities,
    Soils,
    SoilTextureSets,
    PlanetResources,
    ProceduralPlanetAudio,
}

const FAMILIES: [Family; COUNT] = [
    Family::StarmapObjects,
    Family::StarmapTypes,
    Family::StarmapPresets,
    Family::StarmapAmenities,
    Family::Soils,
    Family::SoilTextureSets,
    Family::PlanetResources,
    Family::ProceduralPlanetAudio,
];

const STARMAP: [Family; 4] = [
    Family::StarmapTypes,
    Family::StarmapPresets,
    Family::StarmapAmenities,
    Family::StarmapObjects,
];

impl Family {
    const fn path(self) -> &'static str {
        match self {
            Self::StarmapObjects => "/definitions/starmapObjects",
            Self::StarmapTypes => "/definitions/starmapTypes",
            Self::StarmapPresets => "/definitions/starmapPresets",
            Self::StarmapAmenities => "/definitions/starmapAmenities",
            Self::Soils => "/definitions/soils",
            Self::SoilTextureSets => "/definitions/soilTextureSets",
            Self::PlanetResources => "/definitions/planetResources",
            Self::ProceduralPlanetAudio => "/definitions/proceduralPlanetAudio",
        }
    }

    fn starmap_selection(self) -> TestResult<StarmapDefinitionSelection> {
        let family = match self {
            Self::StarmapObjects => StarmapDefinitionFamily::Objects,
            Self::StarmapTypes => StarmapDefinitionFamily::Types,
            Self::StarmapPresets => StarmapDefinitionFamily::Presets,
            Self::StarmapAmenities => StarmapDefinitionFamily::Amenities,
            _ => return Err("expected one Starmap definition family".into()),
        };
        Ok(StarmapDefinitionSelection::new(
            family,
            CatalogRecordSelection::All,
        ))
    }
}

#[test]
#[ignore = "standalone cold query: configure the installed Build and one World definition route"]
fn installed_world_definitions_cold_query() -> TestResult {
    let path = env::var(COLD_DATASET)?;
    let family = FAMILIES
        .into_iter()
        .find(|family| family.path() == path)
        .ok_or("configure one exact World definition dataset path")?;
    let expected = expected()?;
    equipment_input::with_archive(|archive| {
        let mut proof = Proof::cold(archive, expected);
        read_cold(archive, family, &mut proof)
    })
}

fn read_cold(archive: &Archive, family: Family, proof: &mut Proof<'_>) -> TestResult {
    match family {
        Family::Soils | Family::SoilTextureSets => read_soil(archive, family, proof),
        Family::PlanetResources => proof.loose(
            family,
            &planet_resources::read_planet_resources(archive, PlanetResourceSelection::All, None)?,
            |row| row.identity().entry_index(),
        ),
        Family::ProceduralPlanetAudio => planet_audio::with_procedural_planet_audio(
            archive,
            CatalogRecordSelection::All,
            None,
            |snapshot| observe_audio(&snapshot, proof),
        )?,
        _ => starmap::with_starmap_definitions(
            archive,
            family.starmap_selection()?,
            CatalogLanguageSelection::AllAvailable,
            None,
            |snapshot| observe_starmap(family, &snapshot, proof),
        )?,
    }
}

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_world_definitions_match_one_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    let expected = expected()?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let mut proof = Proof::retained(archive, expected);
        retain(archive, &mut proof)?;
        let compared = proof.compare(measurement)?;
        print_line(&format!(
            "complete World definition SDK parity: rows={:?} sourceUses={} physicalSources={}",
            compared.rows, compared.source_uses, compared.physical_sources,
        ));
        Ok(())
    })
}

fn retain(archive: &Archive, proof: &mut Proof<'_>) -> TestResult {
    read_soil(archive, Family::Soils, proof)?;
    read_soil(archive, Family::SoilTextureSets, proof)?;
    let session = CatalogQuerySession::open(archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let resources = parsed.read_planet_resources(PlanetResourceSelection::All, None)?;
    proof.loose(Family::PlanetResources, &resources, |row| {
        row.identity().entry_index()
    })?;
    parsed.with_procedural_planet_audio(CatalogRecordSelection::All, None, |snapshot| {
        observe_audio(&snapshot, proof)
    })??;
    for family in STARMAP {
        parsed.with_starmap_definitions(family.starmap_selection()?, None, |snapshot| {
            observe_starmap(family, &snapshot, proof)
        })??;
    }
    Ok(())
}

fn read_soil(archive: &Archive, family: Family, proof: &mut Proof<'_>) -> TestResult {
    match family {
        Family::Soils => proof.loose(
            family,
            &soils::read_soils(archive, LooseDefinitionSelection::All, None)?,
            |row| row.identity.entry_index(),
        ),
        Family::SoilTextureSets => proof.loose(
            family,
            &soils::read_soil_texture_sets(archive, LooseDefinitionSelection::All, None)?,
            |row| row.identity.entry_index(),
        ),
        _ => Err("expected one Archive soil definition family".into()),
    }
}

fn observe_audio(
    snapshot: &ProceduralPlanetAudioSnapshot<'_>,
    proof: &mut Proof<'_>,
) -> TestResult {
    let scope = RecordScope {
        build: snapshot.build(),
        availability: snapshot.availability(),
        inventory: snapshot.source_inventory(),
        coverage: snapshot.coverage(),
    };
    proof.records(
        Family::ProceduralPlanetAudio,
        &scope,
        snapshot.rows(),
        |row| row.source().record_index().value().copied(),
    )
}

fn observe_starmap(
    family: Family,
    snapshot: &StarmapDefinitionSnapshot<'_>,
    proof: &mut Proof<'_>,
) -> TestResult {
    let scope = RecordScope {
        build: snapshot.build.as_ref(),
        availability: snapshot.availability,
        inventory: &snapshot.source_inventory,
        coverage: &snapshot.coverage,
    };
    match (family, &snapshot.rows) {
        (Family::StarmapObjects, StarmapDefinitionRows::Objects(rows)) => {
            proof.records(family, &scope, rows, |row| {
                row.source.record_index().value().copied()
            })
        }
        (Family::StarmapTypes, StarmapDefinitionRows::Types(rows)) => {
            proof.records(family, &scope, rows, |row| {
                row.header().record_index().value().copied()
            })
        }
        (Family::StarmapPresets, StarmapDefinitionRows::Presets(rows)) => {
            proof.records(family, &scope, rows, |row| {
                row.header().record_index().value().copied()
            })
        }
        (Family::StarmapAmenities, StarmapDefinitionRows::Amenities(rows)) => {
            proof.records(family, &scope, rows, |row| {
                row.header().record_index().value().copied()
            })
        }
        _ => return Err("the typed Starmap rows do not match the selected family".into()),
    }?;
    proof.texts(snapshot)
}

fn expected() -> TestResult<Expected> {
    Expected::parse(&env::var(EXPECTED_ROWS)?, &env::var(OBJECT_LEFTOVERS)?)
}
