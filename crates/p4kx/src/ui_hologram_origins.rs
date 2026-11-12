// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::{HologramCameraField, ObservedHologramCamera};
use p4kx_extraction::{
    DataForgeEntry, DataForgeOrigin, FieldOrigin, FieldOrigins, FieldPath, InputScope, SourceInput,
    SourceInventory, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
    SourceValueState,
};
use serde::Serialize;

use super::{HologramArgs, Summary};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Evidence<'artifact> {
    field_origins: FieldOrigins,
    provenance: SourceInventory,
    artifact_origin: &'artifact FieldOrigin,
    origin_receipt: &'artifact str,
}

pub(super) struct CameraInput {
    field: &'static str,
    sources: Vec<SourceUse>,
    complete: bool,
}

impl CameraInput {
    pub(super) fn sources(&self) -> &[SourceUse] {
        &self.sources
    }
}

pub(super) type CameraInputs = [CameraInput; 3];

pub(super) struct Projection {
    uses: SourceUses,
    scope: SourceScope,
    fields: FieldOrigins,
}

impl Projection {
    pub(super) fn new(entry: &DataForgeEntry) -> Result<Self> {
        let uses = SourceUses::new();
        let input = match entry.origin() {
            DataForgeOrigin::Archive(entry) => SourceInput::Archive(entry),
            _ => SourceInput::Unknown,
        };
        let scope = uses.observe(input, InputScope::Payload)?;
        Ok(Self {
            uses,
            scope,
            fields: FieldOrigins::new(),
        })
    }

    pub(super) fn finish<'artifact>(
        mut self,
        entity_record: u32,
        camera: &ObservedHologramCamera,
        settings: &CameraInputs,
        summary: &Summary<'_>,
        published: &'artifact super::png_origins::Published,
    ) -> Result<Evidence<'artifact>> {
        self.summary(entity_record, summary)?;
        self.camera(camera, settings)?;
        self.put("/originReceipt", &[], true)?;
        self.fields
            .insert(FieldPath::parse("/bytes")?, published.artifact.clone())?;
        for path in ["/drawn_parts", "/triangles"] {
            self.fields
                .insert(FieldPath::parse(path)?, published.scene.clone())?;
        }
        Ok(Evidence {
            field_origins: self.fields,
            provenance: self.uses.freeze()?,
            artifact_origin: &published.artifact,
            origin_receipt: &published.receipt,
        })
    }

    pub(super) const fn uses(&self) -> &SourceUses {
        &self.uses
    }

    pub(super) const fn scope(&self) -> &SourceScope {
        &self.scope
    }

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
        for path in ["/parts", "/modelled_parts", "/meshes"] {
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

    fn camera(&mut self, original: &ObservedHologramCamera, settings: &CameraInputs) -> Result<()> {
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
        for setting in settings {
            self.put(
                &format!("/camera/{}", setting.field),
                &setting.sources,
                setting.complete,
            )?;
        }
        Ok(())
    }

    pub(super) fn camera_inputs(
        &self,
        original: &ObservedHologramCamera,
        args: &HologramArgs,
    ) -> Result<CameraInputs> {
        Ok([
            self.setting(
                "fov_degrees",
                original,
                HologramCameraField::FieldOfView,
                args.fov.is_some(),
            )?,
            self.setting(
                "view_angle_degrees",
                original,
                HologramCameraField::ViewAngle,
                args.view_angle.is_some(),
            )?,
            self.setting(
                "distance_scaler",
                original,
                HologramCameraField::DistanceScaler,
                args.distance_scaler.is_some(),
            )?,
        ])
    }

    fn setting(
        &self,
        field: &'static str,
        original: &ObservedHologramCamera,
        setting: HologramCameraField,
        overridden: bool,
    ) -> Result<CameraInput> {
        let state = original.field_state(setting);
        let mut input = CameraInput {
            field,
            sources: Vec::new(),
            complete: overridden || state == SourceValueState::Absent,
        };
        if overridden {
            return Ok(input);
        }
        let Some(record_index) = original.record_index() else {
            return Ok(input);
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
        input.sources = vec![used, selected];
        input.complete = complete;
        Ok(input)
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
