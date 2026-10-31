// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::IssueKind;

impl IssueKind {
    /// Returns a stable code for the kind, e.g. `geometry_missing`, for scripts; the message is
    /// text for people.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Unresolved(_) => "unresolved",
            Self::NoGeometry => "no_geometry",
            Self::GeometryMissing { .. } => "geometry_missing",
            Self::SubGeometryMissing { .. } => "sub_geometry_missing",
            Self::BodyGeometryMissing { .. } => "body_geometry_missing",
            Self::WorldPropNotWorn { .. } => "world_prop_not_worn",
            Self::GeometryUnreadable { .. } => "geometry_unreadable",
            Self::MaterialLibrary { .. } => "material_library",
            Self::Palette { .. } => "palette",
            Self::HelperNotFound { .. } => "helper_not_found",
            Self::AttachmentsLeftOut { .. } => "attachments_left_out",
            Self::SkeletonMissing { .. } => "skeleton_missing",
            Self::SkeletonUnreadable { .. } => "skeleton_unreadable",
            Self::GearNotDeployed { .. } => "gear_not_deployed",
            Self::TooManyParts { .. } => "too_many_parts",
            Self::TooManyVertices { .. } => "too_many_vertices",
            Self::TooDeep { .. } => "too_deep",
            Self::ContainerMissing { .. } => "container_missing",
            Self::ContainerUnreadable { .. } => "container_unreadable",
            Self::ContainerCycle { .. } => "container_cycle",
            Self::TooManyContainers { .. } => "too_many_containers",
            Self::FileMissing { .. } => "file_missing",
            Self::FileUnreadable { .. } => "file_unreadable",
            Self::InvalidPlacement { .. } => "invalid_placement",
            Self::InvalidOffset { .. } => "invalid_offset",
            Self::OffsetDiffers { .. } => "offset_differs",
            Self::ObjectsLeftOut { .. } => "objects_left_out",
            Self::UnsupportedObjects { .. } => "unsupported_objects",
            Self::EntitiesLeftOut { .. } => "entities_left_out",
            Self::LightUnsupported { .. } => "light_unsupported",
            Self::BodyZonesHidden(_) => "body_zones_hidden",
            Self::BodyZonesUnknown(_) => "body_zones_unknown",
            Self::ClothingConditionsUndecided(_) => "clothing_conditions_undecided",
            Self::PartHidden { .. } => "part_hidden",
            Self::LayersWithoutDiffuse(_) => "layers_without_diffuse",
            Self::PieceNotFound { .. } => "piece_not_found",
            Self::NoWornGeometry => "no_worn_geometry",
            Self::NoVariantGeometry { .. } => "no_variant_geometry",
            Self::MainGeometryWorn { .. } => "main_geometry_worn",
            Self::BonesNotInSkeleton { .. } => "bones_not_in_skeleton",
            Self::NotSkinned => "not_skinned",
            Self::ShapeFailed { .. } => "shape_failed",
            Self::MaterialChoiceUnknown { .. } => "material_choice_unknown",
            Self::LookIgnored { .. } => "look_ignored",
            Self::BodyTypeMismatch { .. } => "body_type_mismatch",
            Self::FollowMorphsFirstMeshOnly { .. } => "follow_morphs_first_mesh_only",
            Self::FollowWeightsSkipped { .. } => "follow_weights_skipped",
            Self::FollowJointsSkipped { .. } => "follow_joints_skipped",
            Self::LoadoutUnreadable { .. } => "loadout_unreadable",
            Self::MorphsFailed { .. } => "morphs_failed",
            Self::UnknownExpression { .. } => "unknown_expression",
            Self::EmptyExpression { .. } => "empty_expression",
            Self::FaceJointsFailed { .. } => "face_joints_failed",
            Self::FaceAnimationFailed { .. } => "face_animation_failed",
            Self::AnimationNotUsed { .. } => "animation_not_used",
        }
    }

    /// Returns whether the part, or its geometry, is left out of the Assembly because of this.
    #[must_use]
    pub const fn skips(&self) -> bool {
        matches!(
            self,
            Self::Unresolved(_)
                | Self::GeometryMissing { .. }
                | Self::BodyGeometryMissing { .. }
                | Self::WorldPropNotWorn { .. }
                | Self::GeometryUnreadable { .. }
                | Self::TooManyParts { .. }
                | Self::TooManyVertices { .. }
                | Self::TooDeep { .. }
                | Self::ContainerMissing { .. }
                | Self::ContainerUnreadable { .. }
                | Self::ContainerCycle { .. }
                | Self::TooManyContainers { .. }
                | Self::FileMissing { .. }
                | Self::FileUnreadable { .. }
                | Self::InvalidPlacement { .. }
                | Self::ObjectsLeftOut { .. }
                | Self::UnsupportedObjects { .. }
                | Self::EntitiesLeftOut { .. }
                | Self::LightUnsupported { .. }
                | Self::PieceNotFound { .. }
                | Self::NoWornGeometry
                | Self::NoVariantGeometry { .. }
                | Self::ShapeFailed { .. }
                | Self::AnimationNotUsed { .. }
        )
    }
}
