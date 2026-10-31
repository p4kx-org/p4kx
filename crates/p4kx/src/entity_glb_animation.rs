// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The animation lines of the text report of `p4kx entity glb`.

use std::io::{self, BufWriter, Write};

use p4kx_assembly::{CreatureAnimations, CreatureClips};
use p4kx_extraction::GlbSummary;

use crate::entity_glb_report::print_issues;
use crate::model_animations::write_notes;

/// Prints what the GLB wrote of the animations of a creature, what was left out of them, and why
/// some could not be looked for; nothing for an Entity without clips.
pub(crate) fn print_animations(
    summary: &GlbSummary,
    found: &CreatureClips,
    animated: &CreatureAnimations<'_>,
) -> io::Result<()> {
    let mut stdout = BufWriter::new(io::stdout().lock());
    write_notes(&mut stdout, &summary.animations, &found.set)?;
    print_issues(&mut stdout, &found.issues)?;
    print_issues(&mut stdout, animated.issues())?;
    stdout.flush()
}
