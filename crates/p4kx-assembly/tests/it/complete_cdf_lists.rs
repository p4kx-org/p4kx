// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::complete_cdf_source::{self as source, TestResult};
use p4kx_assembly::{ArchiveModels, ModelProvider};

#[test]
fn cdf_complete_keeps_all_65_original_bone_attachments() -> TestResult {
    source::with_archive(65, 0, |archive| {
        let provided = ArchiveModels::new(archive).model(source::CDF, 0)?;
        assert_eq!(
            provided.attachments.len(),
            65,
            "all actual original bones survive the reader"
        );
        for (attachment, index) in provided.attachments.iter().zip(0_u16..65) {
            assert_eq!(
                attachment.name,
                format!("Bone_{index}"),
                "source declaration order"
            );
            assert_eq!(attachment.bone, "tip", "original skeleton binding");
            assert_eq!(
                attachment.position,
                [f32::from(index), 0.0, 0.0],
                "the actual late offset"
            );
        }
        assert!(
            provided.left_out.is_empty(),
            "a source list cap cannot invent an undrawn attachment"
        );
        assert_eq!(
            provided.entry,
            source::SKIN,
            "first-skin model selection is unchanged"
        );
        Ok(())
    })
}

#[test]
fn cdf_complete_lists_all_257_original_unselected_attachments() -> TestResult {
    source::with_archive(0, 257, |archive| {
        let provided = ArchiveModels::new(archive).model(source::CDF, 0)?;
        assert_eq!(
            provided.left_out.len(),
            257,
            "the real source tail remains in the output list"
        );
        let expected: Vec<_> = (0..257)
            .map(|index| format!("Objects/Test/left_{index}.skin"))
            .collect();
        assert_eq!(
            provided.left_out, expected,
            "all exact original bindings retain authored order"
        );
        assert!(
            provided.attachments.is_empty(),
            "unselected skins do not become bone attachments"
        );
        assert_eq!(
            provided.entry,
            source::SKIN,
            "the initial original skin is still selected"
        );
        Ok(())
    })
}

#[test]
fn cdf_complete_small_lists_preserve_selection_and_offsets() -> TestResult {
    source::with_archive(2, 2, |archive| {
        let provided = ArchiveModels::new(archive).model(source::CDF, 0)?;
        assert_eq!(
            provided.attachments.len(),
            2,
            "two readable original attachments"
        );
        let first = provided
            .attachments
            .first()
            .ok_or("first original attachment")?;
        let last = provided
            .attachments
            .last()
            .ok_or("second original attachment")?;
        assert_eq!(first.name, "Bone_0", "first original identity");
        assert_eq!(
            last.name, "Bone_1",
            "repeated source file has a distinct original occurrence"
        );
        assert_eq!(
            last.position,
            [1.0, 0.0, 0.0],
            "distinct original offset survives"
        );
        assert_eq!(
            provided.left_out,
            ["Objects/Test/left_0.skin", "Objects/Test/left_1.skin"],
            "unselected source order"
        );
        assert_eq!(provided.entry, source::SKIN, "first-skin precedence");
        Ok(())
    })
}

#[test]
fn cdf_complete_keeps_vertex_admission_separate_from_source_list_membership() -> TestResult {
    source::with_archive(3, 0, |archive| {
        let models = ArchiveModels::new(archive).with_max_attachment_vertices(60);
        let provided = models.model(source::CDF, 0)?;
        assert_eq!(
            provided.attachments.len(),
            1,
            "two models of24 fit; a third does not"
        );
        assert_eq!(
            provided.left_out,
            ["Objects/Test/housing.cgf", "Objects/Test/housing.cgf"],
            "both actual refused occurrences remain"
        );
        assert_eq!(
            provided
                .attachments
                .first()
                .ok_or("the retained first bone")?
                .name,
            "Bone_0",
            "source order determines vertex admission"
        );
        Ok(())
    })
}
