// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    AuthoredManufacturer, ItemAttachmentPresentation, ItemDefinition, ItemDefinitionComponentKind,
};
use p4kx_export::catalog_queries::{ResourceNumber, SourcePayload};
use p4kx_extraction::{SourceValueState, SourcedValue};

use super::fixture;
use crate::commodity_v3_fixture::{self as common, TestResult};

pub(super) fn verify(row: &ItemDefinition<'_>, layout: fixture::Layout) -> TestResult {
    let components = row.components().value().ok_or("original Item components")?;
    assert_eq!(
        components.len(),
        4,
        "repeated attachments, physics and purchase remain separate"
    );
    for (ordinal, component) in components.iter().enumerate() {
        assert_eq!(
            component.component_ordinal().value(),
            Some(&u32::try_from(ordinal + 1)?),
            "the original null slot does not renumber components"
        );
        match component.body() {
            ItemDefinitionComponentKind::Attachment(value) => attachment(value, layout)?,
            ItemDefinitionComponentKind::Physics(value) => {
                let mass = value
                    .value()
                    .ok_or("retained physics")?
                    .mass()
                    .value()
                    .ok_or("original mass")?;
                let expected = if matches!(layout, fixture::Layout::Nonfinite) {
                    fixture::MASS_NAN
                } else {
                    (-0.0_f32).to_bits()
                };
                assert_eq!(
                    mass.raw_bits(),
                    expected,
                    "exact Single signed-zero or NaN bits survive"
                );
            }
            ItemDefinitionComponentKind::Purchasable(value) => {
                let expected = if matches!(layout, fixture::Layout::MissingFields) {
                    SourceValueState::Absent
                } else {
                    SourceValueState::Present
                };
                assert_eq!(
                    value.display_name().state(),
                    expected,
                    "missing declaration is distinct from an empty key"
                );
            }
            _ => return Err("a new component needs a fixture expectation".into()),
        }
    }
    Ok(())
}

fn attachment(
    value: &SourcedValue<Box<ItemAttachmentPresentation<'_>>>,
    layout: fixture::Layout,
) -> TestResult {
    if matches!(layout, fixture::Layout::NullAttachment) {
        assert_eq!(
            value.state(),
            SourceValueState::Null,
            "the authored null is retained"
        );
        return Ok(());
    }
    let row = value.value().ok_or("retained attachment")?;
    unavailable_fields(row, layout)?;
    if matches!(layout, fixture::Layout::Nonfinite) {
        let ResourceNumber::Double(number) = row.grade().value().ok_or("original Double grade")?
        else {
            return Err("source numeric kind remains Double".into());
        };
        assert_eq!(
            number.raw_bits(),
            fixture::GRADE_NAN,
            "the Double NaN payload remains exact"
        );
    }
    motion(row, layout)?;
    manufacturer(row.manufacturer(), layout)
}

fn unavailable_fields(row: &ItemAttachmentPresentation<'_>, layout: fixture::Layout) -> TestResult {
    if matches!(layout, fixture::Layout::MissingFields) {
        assert_eq!(
            row.grade().state(),
            SourceValueState::Absent,
            "numeric declaration absence survives"
        );
        assert_eq!(
            row.localization().state(),
            SourceValueState::Absent,
            "nested declaration absence survives"
        );
    }
    if matches!(layout, fixture::Layout::UnreadableName) {
        let name = row
            .localization()
            .value()
            .ok_or("readable localization owner")?
            .name();
        assert_eq!(
            name.state(),
            SourceValueState::Unreadable,
            "the invalid original Locale offset is unreadable"
        );
        assert!(
            !name.origin().source_set_complete(),
            "the unreadable original value is incomplete"
        );
    }
    Ok(())
}

fn motion(row: &ItemAttachmentPresentation<'_>, layout: fixture::Layout) -> TestResult {
    if matches!(layout, fixture::Layout::NonWeapon) {
        assert!(
            row.weapon_motion().is_none(),
            "a non-personal weapon gains no motion inputs"
        );
    } else {
        let motion = row
            .weapon_motion()
            .ok_or("authored personal-weapon inputs")?;
        assert_eq!(
            motion.item_type().value(),
            Some(&"WeaponPersonal"),
            "the original type selector survives"
        );
        let expected = if matches!(layout, fixture::Layout::MissingFields) {
            None
        } else {
            Some(&"TagOne TagTwo")
        };
        assert_eq!(
            motion.tags().value(),
            expected,
            "source tags stay unsplit or explicitly absent"
        );
    }
    Ok(())
}

fn manufacturer(value: &AuthoredManufacturer<'_>, layout: fixture::Layout) -> TestResult {
    match (value, layout) {
        (AuthoredManufacturer::Authored(value), fixture::Layout::AuthoredManufacturer) => {
            let arena = value.value().ok_or("authored manufacturer arena")?;
            let [field] = arena.roots() else {
                return Err("one original manufacturer field".into());
            };
            let node = arena.value(field.value()).ok_or("authored root value")?;
            let SourcePayload::Array { elements } = node.value().value().ok_or("original array")?
            else {
                return Err("original array storage".into());
            };
            assert_eq!(
                elements.len(),
                3,
                "repeated Reference and null slots survive"
            );
            let [first, null, last] = elements.as_slice() else {
                return Err("exact three original manufacturer slots".into());
            };
            assert_ne!(
                first.value_index(),
                last.value_index(),
                "equal GUID values keep distinct original occurrences"
            );
            assert_eq!(
                arena.value(*null).ok_or("null slot")?.value().state(),
                SourceValueState::Null,
                "the null retains its original slot"
            );
        }
        (AuthoredManufacturer::Record(value), fixture::Layout::MissingFields) => {
            assert_eq!(
                value.state(),
                SourceValueState::Absent,
                "manufacturer declaration absence survives"
            );
        }
        (AuthoredManufacturer::Record(value), _) => {
            let reference = value.value().ok_or("manufacturer Reference")?;
            assert_eq!(
                reference.guid().value().map(ToString::to_string),
                Some(common::guid(fixture::MAKER)),
                "the stored target GUID survives"
            );
        }
        _ => return Err("manufacturer storage must not be rewritten".into()),
    }
    Ok(())
}
