// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveModels, ModelProvider, ProvidedAttachment, ProvidedModel};

use crate::cdf_attachment_storage_source as material_source;
use crate::complete_cdf_source::{self as source, TestResult};

#[test]
fn cdf_repeated_asset_shares_model_and_keeps_each_original_instance() -> TestResult {
    source::with_archive(2, 0, |archive| {
        let provided = ArchiveModels::new(archive).model(source::CDF, 0)?;
        let (first, last) = instances(&provided)?;
        assert!(
            Arc::ptr_eq(&first.model, &last.model),
            "identical decoded model data is shared without merging instances"
        );
        Ok(())
    })
}

fn instances(
    provided: &ProvidedModel,
) -> Result<(&ProvidedAttachment, &ProvidedAttachment), Box<dyn std::error::Error>> {
    assert_eq!(
        provided.attachments.len(),
        2,
        "both genuine original instances"
    );
    assert!(
        provided.left_out.is_empty(),
        "no instance was deduplicated away"
    );
    let first = provided.attachments.first().ok_or("first genuine bone")?;
    let last = provided.attachments.last().ok_or("second genuine bone")?;
    assert_instance(first, 0);
    assert_instance(last, 1);
    Ok((first, last))
}

fn assert_instance(attachment: &ProvidedAttachment, index: u16) {
    assert_eq!(
        attachment.name,
        format!("Bone_{index}"),
        "original authored identity"
    );
    assert_eq!(
        attachment.position,
        [f32::from(index), 0.0, 0.0],
        "original per-occurrence offset"
    );
    assert_eq!(
        attachment.rotation,
        [1.0, 0.0, 0.0, 0.0],
        "original rotation"
    );
    assert_eq!(attachment.bone, "tip", "genuine bone binding");
    assert_eq!(
        attachment.entry, "Data\\Objects\\Test\\housing.cgf",
        "actual selected canonical Entry"
    );
    assert_eq!(
        attachment.model.material_library.as_deref(),
        Some("Objects/Test/paint"),
        "original model material"
    );
}

#[test]
fn cdf_shared_attachment_models_keep_exact_material_variants_distinct() -> TestResult {
    material_source::with_archive(|archive| {
        let provided = ArchiveModels::new(archive).model(source::CDF, 0)?;
        assert_eq!(
            provided.attachments.len(),
            material_source::MATERIALS.len(),
            "every original material occurrence"
        );
        assert!(
            provided.left_out.is_empty(),
            "all original bindings remain drawable"
        );
        for (attachment, (index, material)) in provided
            .attachments
            .iter()
            .zip(material_source::MATERIALS.iter().enumerate())
        {
            assert_variant(attachment, index, *material)?;
        }
        let red = provided.attachments.first().ok_or("actual red variant")?;
        let blue = provided.attachments.get(1).ok_or("actual blue variant")?;
        let unchanged = provided
            .attachments
            .get(3)
            .ok_or("original default material")?;
        let empty = provided
            .attachments
            .get(5)
            .ok_or("present empty material override")?;
        assert!(
            !Arc::ptr_eq(&red.model, &blue.model),
            "different overrides never alias mutable variant values"
        );
        assert!(
            !Arc::ptr_eq(&unchanged.model, &empty.model),
            "absent and present empty overrides remain distinct"
        );
        Ok(())
    })
}

fn assert_variant(
    attachment: &ProvidedAttachment,
    index: usize,
    material: Option<&str>,
) -> TestResult {
    assert_eq!(
        attachment.name,
        format!("Variant_{index}"),
        "authored instance identity"
    );
    assert_eq!(
        attachment.position,
        [f32::from(u16::try_from(index)?), 0.0, 0.0],
        "original per-instance offset"
    );
    let expected = match material {
        Some("Objects/Test/red.mtl") => Some("Objects/Test/red"),
        Some("Objects/Test/blue.mtl") => Some("Objects/Test/blue"),
        Some("") => Some(""),
        None => Some("Objects/Test/paint"),
        _ => return Err("unexpected source variant".into()),
    };
    assert_eq!(
        attachment.model.material_library.as_deref(),
        expected,
        "exact original override without cross-variant mutation"
    );
    Ok(())
}

#[test]
fn cdf_attachment_storage_keeps_instance_vertex_charges_and_call_local_lifetime() -> TestResult {
    source::with_archive(3, 0, |archive| {
        let provider = ArchiveModels::new(archive).with_max_attachment_vertices(60);
        let first = provider.model(source::CDF, 0)?;
        let second = provider.model(source::CDF, 0)?;
        assert_eq!(
            first.attachments.len(),
            1,
            "skin24 plus one attachment24 fit60"
        );
        assert_eq!(
            first.left_out,
            ["Objects/Test/housing.cgf", "Objects/Test/housing.cgf"],
            "both refused original occurrences retain their order"
        );
        assert_eq!(
            second.left_out, first.left_out,
            "each call charges all original instances"
        );
        let first = first.attachments.first().ok_or("first call model")?;
        let second = second.attachments.first().ok_or("second call model")?;
        assert!(
            !Arc::ptr_eq(&first.model, &second.model),
            "character calls do not retain a global model cache"
        );
        Ok(())
    })
}
