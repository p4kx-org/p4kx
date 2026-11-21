// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::{env, fs};

use p4kx_export::catalog_queries::equipment::component_definitions::{
    ComponentDefinitionSnapshot, ConsumableSubtype, ConsumableType, DamageResistanceMacro,
    MiningLaserGlobalDefinition, TurretHealthModifier,
};
use p4kx_export::catalog_queries::{
    CanonicalRecordHeader, CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, Build, DataForge, DataForgeEntry, DataForgeOrigin, InputScope,
    RecordTree, SourceEncryption, SourceInput, SourceScope, SourceUses,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde::Deserialize;

use crate::real_archive_item_durability::{Trace, indexed};
use crate::real_archive_verification::print_line;

#[path = "definition_sdk_witness_values.rs"]
mod values;

#[path = "definition_sdk_witness_selection.rs"]
mod selection;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Selection {
    build: BuildIdentity,
    damage_resistance_macro: u32,
    mining_laser_global: u32,
    turret_health_modifier: u32,
    consumable_type: u32,
    consumable_subtype: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildIdentity {
    branch: String,
    version: String,
    change_number: String,
    build_id: String,
}

impl BuildIdentity {
    fn capture(observed: &Build) -> TestResult<Self> {
        let identity = Self {
            branch: observed.branch().to_owned(),
            version: observed.version().to_owned(),
            change_number: observed.change_number().to_owned(),
            build_id: observed.build_id().to_owned(),
        };
        if [
            &identity.branch,
            &identity.version,
            &identity.change_number,
            &identity.build_id,
        ]
        .iter()
        .any(|member| member.trim().is_empty())
        {
            return Err("the Archive Build identity must contain all four parts".into());
        }
        Ok(identity)
    }

    fn require(&self, observed: &Build) -> TestResult {
        if self.branch == observed.branch()
            && self.version == observed.version()
            && self.change_number == observed.change_number()
            && self.build_id == observed.build_id()
        {
            Ok(())
        } else {
            Err("the Archive Build does not match the private Definitions witness".into())
        }
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K; optional private P4KX_DEFINITION_SDK_WITNESS"]
fn installed_component_definition_sdk_preserves_selected_original_values() -> TestResult {
    let explicit = read_selection()?;
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let build = archive
        .build()
        .ok_or("the Archive needs an exact Build manifest")?;
    let observed = BuildIdentity::capture(build)?;
    if let Some(explicit) = &explicit {
        explicit.build.require(build)?;
    }
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let entry = DataForgeEntry::read(&archive)?;
    let original = entry.open()?;
    let selection = match explicit {
        Some(explicit) => explicit,
        None => selection::discover(&original, observed)?,
    };
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    let evidence = Evidence::new(&original, session.dataforge_origin(), build)?;
    prove_definitions(&parsed, &evidence, &selection)?;
    watch.check()?;
    print_line(&format!(
        "Definitions SDK source witness: {} {} {} {}; five selected physical Records",
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ));
    Ok(())
}

fn read_selection() -> TestResult<Option<Selection>> {
    let Some(path) = env::var_os("P4KX_DEFINITION_SDK_WITNESS") else {
        return Ok(None);
    };
    let bytes = fs::read(path).map_err(|_| "read private Definitions selectors")?;
    let selection =
        serde_json::from_slice(&bytes).map_err(|_| "parse private Definitions selectors")?;
    Ok(Some(selection))
}

fn prove_definitions(
    sdk: &ParsedCatalogQueries<'_>,
    evidence: &Evidence<'_>,
    selectors: &Selection,
) -> TestResult {
    let index = selectors.mining_laser_global;
    sdk.with_mining_laser_globals(CatalogRecordSelection::RecordIndex(index), None, |result| {
        evidence.verify(
            &result,
            index,
            selection::MINING_LASER_GLOBAL,
            MiningLaserGlobalDefinition::source,
            values::mining,
        )
    })??;
    let index = selectors.damage_resistance_macro;
    sdk.with_damage_resistance_macros(
        CatalogRecordSelection::RecordIndex(index),
        None,
        |result| {
            evidence.verify(
                &result,
                index,
                selection::DAMAGE_RESISTANCE_MACRO,
                DamageResistanceMacro::source,
                values::armor,
            )
        },
    )??;
    let index = selectors.turret_health_modifier;
    sdk.with_turret_health_modifiers(
        CatalogRecordSelection::RecordIndex(index),
        None,
        |result| {
            evidence.verify(
                &result,
                index,
                selection::TURRET_HEALTH_MODIFIER,
                TurretHealthModifier::source,
                values::turret,
            )
        },
    )??;
    prove_consumables(sdk, evidence, selectors)?;
    Ok(())
}

fn prove_consumables(
    sdk: &ParsedCatalogQueries<'_>,
    evidence: &Evidence<'_>,
    selectors: &Selection,
) -> TestResult {
    let index = selectors.consumable_type;
    sdk.with_consumable_types(CatalogRecordSelection::RecordIndex(index), None, |result| {
        evidence.verify(
            &result,
            index,
            selection::CONSUMABLE_TYPE,
            ConsumableType::source,
            values::consumable_type,
        )
    })??;
    let index = selectors.consumable_subtype;
    sdk.with_consumable_subtypes(CatalogRecordSelection::RecordIndex(index), None, |result| {
        evidence.verify(
            &result,
            index,
            selection::CONSUMABLE_SUBTYPE,
            ConsumableSubtype::source,
            values::consumable_subtype,
        )
    })??;
    Ok(())
}

pub(super) fn verify_linked_tool_definitions<'source>(
    sdk: &ParsedCatalogQueries<'_>,
    forge: &'source DataForge<'source>,
    origin: &DataForgeOrigin,
    build: &'source Build,
    globals: impl IntoIterator<Item = u32>,
    subtypes: impl IntoIterator<Item = u32>,
) -> TestResult {
    let evidence = Evidence::new(forge, origin, build)?;
    for index in globals {
        sdk.with_mining_laser_globals(
            CatalogRecordSelection::RecordIndex(index),
            None,
            |result| {
                evidence.verify(
                    &result,
                    index,
                    selection::MINING_LASER_GLOBAL,
                    MiningLaserGlobalDefinition::source,
                    values::mining,
                )
            },
        )??;
    }
    for index in subtypes {
        sdk.with_consumable_subtypes(
            CatalogRecordSelection::RecordIndex(index),
            None,
            |result| {
                evidence.verify(
                    &result,
                    index,
                    selection::CONSUMABLE_SUBTYPE,
                    ConsumableSubtype::source,
                    |row, tree, trace| values::text(row.type_name(), tree.field("typeName"), trace),
                )
            },
        )??;
    }
    Ok(())
}

struct Evidence<'source> {
    forge: &'source DataForge<'source>,
    build: &'source Build,
    scope: SourceScope,
    encrypted: Option<bool>,
}

impl<'source> Evidence<'source> {
    fn new(
        forge: &'source DataForge<'source>,
        origin: &DataForgeOrigin,
        build: &'source Build,
    ) -> TestResult<Self> {
        let DataForgeOrigin::Archive(physical) = origin else {
            return Err("the selected DataForge requires physical Archive Entry evidence".into());
        };
        let sources = SourceUses::new();
        let scope = sources.observe(SourceInput::Archive(physical), InputScope::Payload)?;
        let encrypted = match origin.encryption() {
            SourceEncryption::Encrypted => Some(true),
            SourceEncryption::Unencrypted => Some(false),
            _ => None,
        };
        Ok(Self {
            forge,
            build,
            scope,
            encrypted,
        })
    }

    fn verify<'sdk, Row>(
        &self,
        result: &ComponentDefinitionSnapshot<'sdk, Row>,
        index: u32,
        family: &str,
        header: impl for<'row> FnOnce(&'row Row) -> &'row CanonicalRecordHeader<'sdk>,
        compare: impl for<'record> FnOnce(&Row, &RecordTree<'record>, &Trace<'_, 'record>) -> TestResult,
    ) -> TestResult {
        let tree = self
            .forge
            .record(index)
            .ok_or("the selected physical Record exists")?
            .tree()?;
        assert!(
            tree.type_name() == family,
            "{family} exact original declaration"
        );
        assert!(
            matches!(result.availability(), DatasetAvailability::Available),
            "{family} available declaration"
        );
        let [row] = result.rows() else {
            return Err("one admitted row for the selected RecordIndex".into());
        };
        let source = header(row);
        assert!(
            source.record_index().value() == Some(&index),
            "{family} source index"
        );
        assert!(
            source.record_path().value() == Some(&tree.record_path()),
            "{family} source path"
        );
        assert!(
            source.guid().value() == Some(&tree.guid()),
            "{family} original GUID"
        );
        assert!(
            source.source_type().value() == Some(&family),
            "{family} exact row declaration"
        );
        assert_eq!(
            result.build(),
            Some(self.build),
            "{family} same Archive Build"
        );
        let coverage = result.coverage();
        assert!(
            coverage.unreadable_headers.is_empty()
                && coverage.unreadable_bodies.is_empty()
                && coverage.incomplete_fields.is_empty(),
            "{family} selected positive definition has complete header, body and field coverage"
        );
        assert!(
            result.definition_discovery().is_complete(),
            "{family} complete selected discovery"
        );

        let inventory = serde_json::to_value(result.source_inventory())?;
        let uses = indexed(&inventory, "/uses", "id")?;
        let trace = Trace::of(&tree, &uses, &self.scope, self.encrypted);
        trace.header(
            &serde_json::to_value(source.record_index())?,
            "recordIndex",
            "selector",
        )?;
        compare(row, &tree, &trace)
    }
}
