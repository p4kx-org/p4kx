// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[derive(Default)]
pub(crate) struct PlaylistDepth {
    open: Vec<u32>,
}

impl PlaylistDepth {
    pub(crate) fn advance(&mut self, children: u32) -> usize {
        while self.open.last() == Some(&0) {
            self.open.pop();
        }
        let depth = self.open.len();
        if let Some(left) = self.open.last_mut() {
            *left = left.saturating_sub(1);
        }
        if children > 0 {
            self.open.push(children);
        }
        depth
    }
}
