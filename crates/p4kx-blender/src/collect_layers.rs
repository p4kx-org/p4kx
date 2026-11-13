// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ModelProvider;
use p4kx_model::{Material, MaterialLayer};

use crate::PackageResult;
use crate::describe::layer_shares;
use crate::manifest::{Issue, IssueOrigin};

use super::{Collected, LibraryInput, MAX_LAYER_DEPTH, id_of};

impl Collected {
    pub(super) fn follow_layers(
        &mut self,
        at: usize,
        material: &Material,
        models: &dyn ModelProvider,
        depth: usize,
    ) -> PackageResult<()> {
        if let Some(parent) = self.descriptions.get_mut(at) {
            parent.followed_at = Some(depth);
        }
        self.cut.remove(&at);
        let mut resolved = vec![false; material.layers.len()];
        for (number, layer) in material.layers.iter().enumerate() {
            let found = self.layer_material(material, layer, models, depth)?;
            if let Some(found) = found
                && let Some(slot) = self
                    .descriptions
                    .get_mut(at)
                    .and_then(|parent| parent.description.layers.get_mut(number))
            {
                slot.material = Some(id_of(found));
                if let Some(flag) = resolved.get_mut(number) {
                    *flag = true;
                }
            }
        }
        if let Some(parent) = self.descriptions.get_mut(at) {
            parent.description.layer_shares = layer_shares(material, |number| {
                resolved.get(number).copied().unwrap_or(false)
            });
        }
        Ok(())
    }

    pub(super) fn cut_issues(&mut self) {
        for (path, name) in std::mem::take(&mut self.cut).into_values() {
            self.issues.push(Issue {
                origin: IssueOrigin::Material,
                kind: "layer_material".to_owned(),
                path,
                name,
                skipped: false,
                reason: format!("layers are followed {MAX_LAYER_DEPTH} files deep"),
            });
        }
    }

    fn layer_material(
        &mut self,
        parent: &Material,
        layer: &MaterialLayer,
        models: &dyn ModelProvider,
        depth: usize,
    ) -> PackageResult<Option<usize>> {
        let path = layer.attribute("Path").map_or_default(str::trim);
        if path.is_empty() {
            return Ok(None);
        }
        let (status, entry) = models.layer_library_observed(path, &mut self.layer_budget);
        let Some(library) = status.library() else {
            let problem = status.problem().unwrap_or_default();
            self.layer_issue(parent, path, &problem);
            return Ok(None);
        };
        let Some(position) = library.layer_position(layer.attribute("Submtl").unwrap_or_default())
        else {
            self.layer_issue(parent, path, "the layer file has no such sub-material");
            return Ok(None);
        };
        let input = LibraryInput {
            library,
            entry: entry.as_ref(),
        };
        self.describe(input, position, None, models, depth + 1)
    }

    fn layer_issue(&mut self, parent: &Material, path: &str, reason: &str) {
        let first =
            self.layer_issues
                .insert((path.to_owned(), parent.name.clone(), reason.to_owned()));
        if !first {
            return;
        }
        self.issues.push(Issue {
            origin: IssueOrigin::Material,
            kind: "layer_material".to_owned(),
            path: path.to_owned(),
            name: parent.name.clone(),
            skipped: false,
            reason: reason.to_owned(),
        });
    }
}
