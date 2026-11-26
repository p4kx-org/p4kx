// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::interaction_source::InteractionList;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const SOURCE_TYPE: &str = "SGripContainer";
const GRIP_TYPE: &str = "SCarryableGrip";
const HAND_GRIP_TYPE: &str = "SGrip";
const PARAMETERS_TYPE: &str = "SCarryParams";

/// The authored grip container and its ordered grip declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CarryableGripContainer<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    grips: InteractionList<CarryableGrip<'source>>,
}

impl<'source> CarryableGripContainer<'source> {
    /// Borrows the original grip array and its ordered slots.
    #[must_use]
    pub const fn grips(&self) -> &SourcedValue<Vec<SourcedValue<CarryableGrip<'source>>>> {
        &self.grips
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let grips = source.class_array_to("grips", GRIP_TYPE, notes, CarryableGrip::read)?;
        Ok(Self {
            source: UnitSource::read(source, [("grips", grips.state())])?,
            grips,
        })
    }
}

/// One authored carryable grip with its declared style, hands and dimensions.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CarryableGrip<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(rename = "carryStyleID")]
    carry_style_id: SourcedValue<&'source str>,
    hand_grip: SourcedValue<CarryableHandGrip<'source>>,
    off_hand_grip: SourcedValue<CarryableHandGrip<'source>>,
    carry_parameters: SourcedValue<CarryableParameters<'source>>,
}

impl<'source> CarryableGrip<'source> {
    /// Borrows the authored carry-style identifier.
    #[must_use]
    pub const fn carry_style_id(&self) -> &SourcedValue<&'source str> {
        &self.carry_style_id
    }

    /// Borrows the inline hand grip.
    #[must_use]
    pub const fn hand_grip(&self) -> &SourcedValue<CarryableHandGrip<'source>> {
        &self.hand_grip
    }

    /// Borrows the original off-hand grip pointer cell.
    #[must_use]
    pub const fn off_hand_grip(&self) -> &SourcedValue<CarryableHandGrip<'source>> {
        &self.off_hand_grip
    }

    /// Borrows the original carry dimensions.
    #[must_use]
    pub const fn carry_parameters(&self) -> &SourcedValue<CarryableParameters<'source>> {
        &self.carry_parameters
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let carry_style_id = source.borrowed_string("carryStyleID", notes)?;
        let hand_grip =
            source.declared_class_to("handGrip", HAND_GRIP_TYPE, notes, CarryableHandGrip::read)?;
        let off_hand_grip = source.strong_pointer_to(
            "offHandGrip",
            HAND_GRIP_TYPE,
            &[HAND_GRIP_TYPE],
            notes,
            CarryableHandGrip::read,
        )?;
        let carry_parameters = source.declared_class_to(
            "carryParameters",
            PARAMETERS_TYPE,
            notes,
            CarryableParameters::read,
        )?;
        Ok(Self {
            source: UnitSource::read(
                source,
                [
                    ("carryStyleID", carry_style_id.state()),
                    ("handGrip", hand_grip.state()),
                    ("offHandGrip", off_hand_grip.state()),
                    ("carryParameters", carry_parameters.state()),
                ],
            )?,
            carry_style_id,
            hand_grip,
            off_hand_grip,
            carry_parameters,
        })
    }
}

/// One authored hand grip and its optional helper string.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CarryableHandGrip<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    optional_helper: SourcedValue<&'source str>,
}

impl<'source> CarryableHandGrip<'source> {
    /// Borrows the original helper name.
    #[must_use]
    pub const fn optional_helper(&self) -> &SourcedValue<&'source str> {
        &self.optional_helper
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let optional_helper = source.borrowed_string("optionalHelper", notes)?;
        Ok(Self {
            source: UnitSource::read(source, [("optionalHelper", optional_helper.state())])?,
            optional_helper,
        })
    }
}

/// The original single-precision carry dimensions.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CarryableParameters<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    width: SourcedValue<ResourceFloat32>,
    height: SourcedValue<ResourceFloat32>,
    depth: SourcedValue<ResourceFloat32>,
}

impl CarryableParameters<'_> {
    /// Borrows the original width.
    #[must_use]
    pub const fn width(&self) -> &SourcedValue<ResourceFloat32> {
        &self.width
    }

    /// Borrows the original height.
    #[must_use]
    pub const fn height(&self) -> &SourcedValue<ResourceFloat32> {
        &self.height
    }

    /// Borrows the original depth.
    #[must_use]
    pub const fn depth(&self) -> &SourcedValue<ResourceFloat32> {
        &self.depth
    }

    fn read<'source>(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<CarryableParameters<'source>, CatalogError> {
        let width = source.single("width", notes)?;
        let height = source.single("height", notes)?;
        let depth = source.single("depth", notes)?;
        Ok(CarryableParameters {
            source: UnitSource::read(
                source,
                [
                    ("width", width.state()),
                    ("height", height.state()),
                    ("depth", depth.state()),
                ],
            )?,
            width,
            height,
            depth,
        })
    }
}
