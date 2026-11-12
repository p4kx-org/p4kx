// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::{HologramCameraField, ObservedHologramCamera};
use p4kx_extraction::{
    DataForgeEntry, DataForgeOrigin, FieldOrigins, FieldPath, InputScope, SourceInput,
    SourceInventory, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
    SourceValueState,
};
use serde::Serialize;

use super::Summary;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Evidence {
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

impl Evidence {
    pub(super) fn of(
        entry: &DataForgeEntry,
        entity_record: u32,
        camera: &ObservedHologramCamera,
        summary: &Summary<'_>,
    ) -> Result<Self> {
        let uses = SourceUses::new();
        let input = match entry.origin() {
            DataForgeOrigin::Archive(entry) => SourceInput::Archive(entry),
            _ => SourceInput::Unknown,
        };
        let scope = uses.observe(input, InputScope::Payload)?;
        let mut projection = Projection {
            uses,
            scope,
            fields: FieldOrigins::new(),
        };
        projection.summary(entity_record, summary)?;
        projection.camera(camera, summary)?;
        Ok(Self {
            field_origins: projection.fields,
            provenance: projection.uses.freeze()?,
        })
    }
}

struct Projection {
    uses: SourceUses,
    scope: SourceScope,
    fields: FieldOrigins,
}

impl Projection {
    fn summary(&mut self, entity: u32, summary: &Summary<'_>) -> Result<()> {
        for path in [
            "/archive",
            "/out",
            "/width",
            "/height",
            "/adapter",
            "/milliseconds",
            "/milliseconds/assembly",
            "/milliseconds/scene",
            "/milliseconds/draw",
            "/milliseconds/write",
            "/milliseconds/total",
        ] {
            self.put(path, &[], true)?;
        }
        for (path, field, role) in [
            ("/entity", "recordName", SourceUseRole::Conversion),
            ("/guid", "guid", SourceUseRole::Payload),
        ] {
            let original = self.header(entity, field, role)?;
            self.put(path, &[original], true)?;
        }
        for path in [
            "/bytes",
            "/parts",
            "/modelled_parts",
            "/drawn_parts",
            "/meshes",
            "/triangles",
        ] {
            self.put(path, &[], false)?;
        }
        self.diagnostics(summary)
    }

    fn diagnostics(&mut self, summary: &Summary<'_>) -> Result<()> {
        let ignored = summary.applied.ignored();
        if !ignored.is_empty() {
            self.put("/ignored", &[], false)?;
            for index in 0..ignored.len() {
                self.put(&format!("/ignored/{index}"), &[], false)?;
            }
        }
        let mismatches = summary.applied.mismatches();
        if !mismatches.is_empty() {
            self.rows(
                "incompatible",
                mismatches.len(),
                &["port", "entity", "kind", "reason"],
            )?;
        }
        self.rows(
            "issues",
            summary.assembly.issues().len(),
            &["path", "name", "kind", "skipped", "reason"],
        )
    }

    fn rows(&mut self, name: &str, count: usize, fields: &[&str]) -> Result<()> {
        self.put(&format!("/{name}"), &[], false)?;
        for index in 0..count {
            let path = format!("/{name}/{index}");
            self.put(&path, &[], false)?;
            for field in fields {
                self.put(&format!("{path}/{field}"), &[], false)?;
            }
        }
        Ok(())
    }

    fn camera(&mut self, original: &ObservedHologramCamera, summary: &Summary<'_>) -> Result<()> {
        if let Some(index) = original.record_index() {
            let read = original.camera().is_some();
            let role = if read {
                SourceUseRole::Selector
            } else {
                SourceUseRole::Attempted
            };
            let sources = [
                self.header(index, "sourceType", role)?,
                self.header(index, "recordName", role)?,
            ];
            self.put("/camera/source", &sources, read)?;
        } else {
            self.put("/camera/source", &[], true)?;
        }
        self.put("/camera", &[], false)?;
        self.put("/camera/elevation_degrees", &[], true)?;
        for (name, setting, overridden) in [
            (
                "fov_degrees",
                HologramCameraField::FieldOfView,
                summary.args.fov.is_some(),
            ),
            (
                "view_angle_degrees",
                HologramCameraField::ViewAngle,
                summary.args.view_angle.is_some(),
            ),
            (
                "distance_scaler",
                HologramCameraField::DistanceScaler,
                summary.args.distance_scaler.is_some(),
            ),
        ] {
            self.setting(&format!("/camera/{name}"), original, setting, overridden)?;
        }
        Ok(())
    }

    fn setting(
        &mut self,
        path: &str,
        original: &ObservedHologramCamera,
        setting: HologramCameraField,
        overridden: bool,
    ) -> Result<()> {
        let state = original.field_state(setting);
        if overridden {
            return self.put(path, &[], true);
        }
        let Some(record_index) = original.record_index() else {
            return self.put(path, &[], state == SourceValueState::Absent);
        };
        let (role, complete) = match state {
            SourceValueState::Present => (SourceUseRole::Conversion, true),
            SourceValueState::Absent | SourceValueState::Invalid | SourceValueState::Null => {
                (SourceUseRole::Selector, true)
            }
            _ => (SourceUseRole::Attempted, false),
        };
        let used = if state == SourceValueState::Absent {
            self.scope.at(
                SourceLocation::Subresource {
                    namespace: "dataforgeRecordDeclarations".to_owned(),
                    indices: vec![u64::from(record_index)],
                    field: "$fields".to_owned(),
                },
                role,
            )?
        } else if original.camera().is_some() {
            self.scope.at(
                SourceLocation::DataForge {
                    record_index,
                    field: setting.name().to_owned(),
                },
                role,
            )?
        } else {
            self.header(record_index, "sourceType", role)?
        };
        let selection = if complete {
            SourceUseRole::Selector
        } else {
            SourceUseRole::Attempted
        };
        let selected = self.header(record_index, "recordName", selection)?;
        self.put(path, &[used, selected], complete)
    }

    fn header(&self, record: u32, field: &str, role: SourceUseRole) -> Result<SourceUse> {
        Ok(self.scope.at(
            SourceLocation::Subresource {
                namespace: "dataforgeRecordHeader".to_owned(),
                indices: vec![u64::from(record)],
                field: field.to_owned(),
            },
            role,
        )?)
    }

    fn put(&mut self, path: &str, sources: &[SourceUse], complete: bool) -> Result<()> {
        Ok(self
            .fields
            .insert(FieldPath::parse(path)?, self.uses.origin(sources, complete))?)
    }
}
