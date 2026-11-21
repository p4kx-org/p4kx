// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};

use super::Summary;
use crate::query_table::one_line;

impl Summary<'_> {
    pub(super) fn print(&self) -> io::Result<()> {
        let (view, source) = self.camera;
        let mut out = BufWriter::new(io::stdout().lock());
        writeln!(
            out,
            "wrote {} of {}: {} x {} px, {} bytes",
            self.args.out.display(),
            one_line(self.class_name),
            self.picture.width(),
            self.picture.height(),
            self.bytes
        )?;
        writeln!(
            out,
            "{} parts ({} with models, {} drawn), {} meshes, {} triangles",
            self.assembly.parts().len(),
            self.assembly.modelled_parts(),
            self.scene.drawn_parts(),
            self.scene.meshes(),
            self.scene.placed_triangles()
        )?;
        writeln!(
            out,
            "camera from {source}: field of view {} degrees, view angle {}, elevation {}, distance factor {}",
            view.fov_degrees, view.view_angle_degrees, view.elevation_degrees, view.distance_scaler
        )?;
        writeln!(
            out,
            "drawn on {} in {} ms (assembly {} ms, scene {} ms, picture {} ms, file {} ms)",
            one_line(self.adapter),
            self.times.total,
            self.times.assembly,
            self.times.scene,
            self.times.draw,
            self.times.write
        )?;
        let (skipped, noted): (Vec<_>, Vec<_>) = self
            .assembly
            .issues()
            .iter()
            .partition(|issue| issue.kind().skips());
        for issue in &skipped {
            let label = if issue.path().is_empty() {
                issue.name().to_owned()
            } else {
                format!("{} ({})", issue.path(), issue.name())
            };
            writeln!(
                out,
                "skipped {}: {}",
                one_line(&label),
                one_line(&issue.kind().to_string())
            )?;
        }
        if !noted.is_empty() {
            writeln!(out, "{} parts have notes, which --json lists", noted.len())?;
        }
        out.flush()
    }
}
