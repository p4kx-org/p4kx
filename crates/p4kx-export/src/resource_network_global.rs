// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::prepared_definition::PreparedDefinition;
use crate::record_observation::RecordObservation;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "resource_network_global_power.rs"]
mod power;

use power::{DefaultPowerDistribution, PowerParameters};

pub(crate) const SOURCE_TYPE: &str = "ItemResourceNetworkGlobal";

const POWER: &str = "powerParams";
const DISTRIBUTION: &str = "defaultPowerDistributionParams";
const COOLANT_RATIO: &str = "maxPowerToCoolantRatio";
const COOLANT_MULTIPLIER: &str = "thrusterCoolantMultiplier";
const BASE_CONVERSION: &str = "powerBaseConversionRate";
const REPAIR_GRACE: &str = "misfireRepairGracePeriod";

/// Original global resource network settings and owned power distribution parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ResourceNetworkGlobal<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    power_params: SourcedValue<PowerParameters<'source>>,
    default_power_distribution_params: SourcedValue<DefaultPowerDistribution<'source>>,
    max_power_to_coolant_ratio: SourcedValue<ResourceFloat32>,
    thruster_coolant_multiplier: SourcedValue<ResourceFloat32>,
    power_base_conversion_rate: SourcedValue<ResourceFloat32>,
    misfire_repair_grace_period: SourcedValue<ResourceFloat32>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourceNetworkGlobal<'source> {
    fn project(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let power_params = fields.declared_class_to(
            POWER,
            "ItemResourceNetworkPowerParams",
            notes,
            PowerParameters::read,
        )?;
        let default_power_distribution_params = fields.declared_class_to(
            DISTRIBUTION,
            "ItemResourceNetworkDefaultPowerDistributionParams",
            notes,
            DefaultPowerDistribution::read,
        )?;
        let max_power_to_coolant_ratio = fields.single(COOLANT_RATIO, notes)?;
        let thruster_coolant_multiplier = fields.single(COOLANT_MULTIPLIER, notes)?;
        let power_base_conversion_rate = fields.single(BASE_CONVERSION, notes)?;
        let misfire_repair_grace_period = fields.single(REPAIR_GRACE, notes)?;
        let additional_fields = fields.authored_members_except([
            (POWER, power_params.state()),
            (DISTRIBUTION, default_power_distribution_params.state()),
            (COOLANT_RATIO, max_power_to_coolant_ratio.state()),
            (COOLANT_MULTIPLIER, thruster_coolant_multiplier.state()),
            (BASE_CONVERSION, power_base_conversion_rate.state()),
            (REPAIR_GRACE, misfire_repair_grace_period.state()),
        ])?;
        Ok(Self {
            source,
            power_params,
            default_power_distribution_params,
            max_power_to_coolant_ratio,
            thruster_coolant_multiplier,
            power_base_conversion_rate,
            misfire_repair_grace_period,
            additional_fields,
        })
    }
}

pub(crate) type PreparedResourceNetworkGlobal<'source> =
    PreparedDefinition<'source, ResourceNetworkGlobal<'source>>;

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(PreparedResourceNetworkGlobal<'source>, Vec<FieldError>), CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        |source, fields, _, notes| ResourceNetworkGlobal::project(source, fields, notes),
    )
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<PreparedResourceNetworkGlobal<'source>>,
) -> CatalogTable<ResourceNetworkGlobal<'source>> {
    PreparedDefinition::order(&mut rows);
    PreparedDefinition::table(forge, SOURCE_TYPE, rows)
}

pub(crate) fn sources<'rows, 'source>(
    rows: &'rows [PreparedResourceNetworkGlobal<'source>],
) -> impl Iterator<Item = (Kind, Guid, &'source str, &'rows [CapturedNumber])> {
    PreparedDefinition::sources(Kind::ResourceNetworkGlobals, rows)
}
