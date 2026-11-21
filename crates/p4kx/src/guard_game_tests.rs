// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::thread;

use super::completed_game_check;

#[test]
fn failed_background_probe_cannot_establish_game_absence() {
    let probe = thread::spawn(|| panic!("synthetic process probe failure"));
    assert!(
        completed_game_check(probe),
        "a failed background probe must preserve the conservative game warning"
    );
}

#[test]
fn completed_background_probes_keep_their_observed_result() {
    for observed in [false, true] {
        let probe = thread::spawn(move || observed);
        assert_eq!(
            completed_game_check(probe),
            observed,
            "a successful probe keeps its actual process result"
        );
    }
}
