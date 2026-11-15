// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::System;

/// The most lines the report keeps per star system of the containers read, and of the issues.
const REPORT_LINES: usize = 300;

/// The longest line the report keeps.
const LINE_LEN: usize = 300;

impl System<'_> {
    /// Keeps `line` in the report of the issues while there is room.
    pub(super) fn issue(&mut self, line: &str) {
        if self.read.report.issues.len() < REPORT_LINES {
            self.read
                .report
                .issues
                .push(line.chars().take(LINE_LEN).collect());
        }
    }

    pub(super) fn report_placements(&mut self, before: [usize; 3], label: &str) {
        if self.seen[1] > before[1] {
            self.read.named += 1;
            if self.read.report.containers.len() < REPORT_LINES {
                self.read.report.containers.push(format!(
                    "{label}: {} children, {} name a record, {} external",
                    self.seen[0] - before[0],
                    self.seen[1] - before[1],
                    self.seen[2] - before[2]
                ));
            }
        }
    }
}
