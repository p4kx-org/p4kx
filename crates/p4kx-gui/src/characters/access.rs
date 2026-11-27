// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{Armour, BodyVariant};
use p4kx_settings::ModelMaterials;

use super::{CharacterExport, CharactersTab, Chooser, HeadInfo, Pick, Sources, SourcesState};
use crate::output_target::Witness;
use crate::{Camera, PartIssue, Playback, Scene, ViewFailure, ViewProgress};

impl CharactersTab {
    pub(crate) fn export_subject(&self) -> Option<Witness> {
        self.shown.as_ref().map(|shown| Witness::of(&shown.armour))
    }

    /// Returns the body variant the Character is built on: the head's, else the chosen one.
    #[must_use]
    pub fn body(&self) -> BodyVariant {
        self.head
            .as_ref()
            .map_or(self.body, |head| head.info.variant)
    }

    /// Returns the head, if a CHF is chosen.
    #[must_use]
    pub fn head(&self) -> Option<&HeadInfo> {
        self.head.as_ref().map(|head| &head.info)
    }

    /// Returns whether a CHF is being read.
    #[must_use]
    pub const fn is_reading_head(&self) -> bool {
        self.head_load.is_some()
    }

    /// Returns why the CHF last chosen cannot be used.
    #[must_use]
    pub fn head_failure(&self) -> Option<&str> {
        self.head_failure.as_deref()
    }

    /// Returns the pieces worn on top of the Loadout.
    #[must_use]
    pub fn pieces(&self) -> &[Pick] {
        &self.pieces
    }

    /// Returns the path of the XML Loadout the pieces are taken from.
    #[must_use]
    pub fn loadout(&self) -> Option<&str> {
        self.loadout.as_deref()
    }

    /// Returns the CHF files and Loadouts of the Archive once they are listed.
    #[must_use]
    pub const fn sources(&self) -> Option<&Sources> {
        match &self.sources {
            SourcesState::Ready(sources) => Some(sources),
            _ => None,
        }
    }

    /// Returns the open choice of a piece, a Loadout or a head.
    #[must_use]
    pub const fn chooser(&self) -> Option<&Chooser> {
        self.chooser.as_ref()
    }

    /// Returns the open choice for changes.
    pub const fn chooser_mut(&mut self) -> Option<&mut Chooser> {
        self.chooser.as_mut()
    }

    /// Returns the level of detail the Character is built and exported at.
    #[must_use]
    pub const fn lod(&self) -> u32 {
        self.lod
    }

    /// Returns how the materials are written to the GLB.
    #[must_use]
    pub const fn materials(&self) -> ModelMaterials {
        self.export_choices.materials
    }

    /// Returns the Armour the view shows.
    #[must_use]
    pub fn armour(&self) -> Option<&Armour> {
        self.shown.as_ref().map(|shown| &*shown.armour)
    }

    /// Returns the scene the view draws: the Armour, with the expression applied.
    #[must_use]
    pub fn scene(&self) -> Option<&Arc<Scene>> {
        self.shown.as_ref().map(|shown| &shown.scene)
    }

    /// Returns what could not be fitted or done in full, in the order of the pieces.
    #[must_use]
    pub fn issues(&self) -> &[PartIssue] {
        self.shown.as_ref().map_or(&[], |shown| &shown.issues)
    }

    /// Returns the pieces of the Loadout that a piece of the same item type took the place of.
    #[must_use]
    pub fn replaced(&self) -> &[p4kx_assembly::Replaced] {
        self.shown.as_ref().map_or(&[], |shown| &shown.replaced)
    }

    /// Returns how far the build that runs has come, or `None` when none runs.
    #[must_use]
    pub fn progress(&self) -> Option<ViewProgress> {
        self.building
            .as_ref()
            .map(|building| building.shared.progress())
    }

    /// Returns whether the Character is being built.
    #[must_use]
    pub const fn is_building(&self) -> bool {
        self.building.is_some()
    }

    /// Returns why the last build gave nothing, or `None`.
    #[must_use]
    pub const fn failure(&self) -> Option<&ViewFailure> {
        self.failure.as_ref()
    }

    /// Returns whether the last build was stopped by the user.
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        self.cancelled && self.building.is_none()
    }

    /// Returns the camera once a scene was built.
    #[must_use]
    pub const fn camera(&self) -> Option<&Camera> {
        self.camera.as_ref()
    }

    /// Returns the camera for moving it.
    pub const fn camera_mut(&mut self) -> Option<&mut Camera> {
        self.camera.as_mut()
    }

    /// Returns the skeleton overlay and animation playback of the Character.
    #[must_use]
    pub const fn playback(&self) -> Option<&Playback> {
        self.playback.as_ref()
    }

    /// Returns the playback for controlling it.
    pub const fn playback_mut(&mut self) -> Option<&mut Playback> {
        self.playback.as_mut()
    }

    /// Returns the export that runs or ended.
    #[must_use]
    pub const fn export(&self) -> Option<&CharacterExport> {
        self.export.as_ref()
    }

    /// Returns the name of the Character: the CHF's file stem, else the body variant.
    #[must_use]
    pub fn name(&self) -> String {
        self.head.as_ref().map_or_else(
            || self.body.code().to_owned(),
            |head| {
                head.info
                    .name
                    .rsplit_once('.')
                    .map_or(head.info.name.as_str(), |(stem, _)| stem)
                    .to_owned()
            },
        )
    }
}
