// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use super::inspect;

#[test]
fn observes_the_exact_guest_image_with_spaces_and_windows_separators() -> io::Result<()> {
    let listing = br"
        41 (wine64-preloader)
        42 C:\Program Files\Roberts Space Industries\StarCitizen\LIVE\Bin64\starcitizen.EXE
        43 /Applications/CrossOver.app/Contents/MacOS/CrossOver
    ";
    assert!(
        inspect(listing)?,
        "the actual guest executable establishes presence despite incomplete other evidence"
    );
    Ok(())
}

#[test]
fn launcher_other_wine_guests_and_zombies_are_not_game_processes() -> io::Result<()> {
    let listing = br"
        44 C:\Program Files\Roberts Space Industries\RSI Launcher\RSI Launcher.exe
        45 C:\windows\system32\wineboot.exe
        46 /Applications/CrossOver.app/Contents/MacOS/CrossOver
        47 /Applications/Wine Stable.app/Contents/Resources/wine/bin/wine64-preloader
        48 C:\Tooling\NotStarCitizen.exe
        49 <defunct>
        50 (kernel_task)
    ";
    assert!(
        !inspect(listing)?,
        "a complete list of other processes establishes absence"
    );
    Ok(())
}

#[test]
fn malformed_rows_and_hidden_wine_images_cannot_establish_absence() {
    for listing in [
        b"".as_slice(),
        b"PID COMMAND\n51 /bin/ps\n",
        b"52 (wine64-preloade)\n53 /bin/ps\n",
        b"54\n55 /bin/ps\n",
    ] {
        assert!(
            matches!(inspect(listing), Err(error) if error.kind() == io::ErrorKind::InvalidData),
            "incomplete observations stay unavailable"
        );
    }
}
