# macOS desktop verification

Run the required checks from [Native macOS support](macos-support.md) on Apple Silicon with
macOS 26 and the current stable macOS release. Use the exact candidate app and extension ZIP.
Record the OS version, architecture, source commit, binary and artifact hashes, and check outcomes.
Keep the device record separate from hosted synthetic CI evidence.

## Hosted synthetic checks

The packaged GUI frame smoke requires the adapter of the actual macOS window to report Metal.
The fresh-process settings tests exercise the standard Library locations, Unicode profile paths,
and the explicit profile root. The GUI interaction suite exercises the application shortcuts,
clipboard commands, selection and audio transport. The device-open failure test exercises the
player's no-device error path. These tests supply synthetic CI evidence.

## Desktop session

Use the unencrypted `native-smoke.p4k` and companion `hello.txt` from the `native_fixture`
example in `p4kx-testkit`. Keep the fixture and any exports in a disposable user-owned directory.

- Launch the installed app from Finder and open the fixture using the native Archive picker.
  Cancel a second open dialog and check that the selected Archive and preview stay open.
- Select `Data/hello.txt` and use the Copy action in the text preview. In a UTF-8 locale, compare
  the system clipboard against the generated reference file with
  `/usr/bin/pbpaste | /usr/bin/cmp - "<fixture>/hello.txt"`. The comparison covers line endings,
  the tab and the combining character.
- Check the Entry's checkbox and press Command+E. Check that it opens the Extraction setup for that
  selection. Cancel a native output picker and verify that no output is created. Complete the
  same setup in a writable Unicode path and compare the extracted bytes. Choose a disposable
  directory without write permission for the negative case, and check that the failure appears
  and the requested output file is absent.
- On an external volume, repeat opening and reading a synthetic Archive with a Unicode path.
  Cover case-sensitive and case-insensitive APFS with disposable fixtures and record the actual
  volume type and observed result. Keep installed game and personal files outside this test.
- In an isolated profile, change a user setting, exit and relaunch. Confirm that the setting is
  retained and the configuration, cache and data paths match [Profile folders](profile-folders.md).
  The clean-host installation and upgrade checks record the installed bundle and profile behavior.
- Run `cargo test -p p4kx-gui --lib coreaudio_consumes_samples_after_start_and_seek_and_stops_when_closed -- --ignored`
  on a Mac with an available output device. The test uses the production output and a quiet
  synthetic tone. In the installed app, open `native-desktop-audio.p4k` from the same fixture
  generator and play `Data/Sounds/tone.wem` in the Audio tab. Seek to the middle and confirm
  playback resumes. Select `Data/readme.txt` to close the preview and confirm playback stops.
  Record an actual unavailable-device observation separately from the synthetic no-device test.

## Installed Blender extension

Run the following on each required device using native Blender 5.2 LTS and the exact release ZIP.
The work directory must be new.

```text
python3 blender_addon/tests/native_extension_smoke.py --blender "/Applications/Blender.app/Contents/MacOS/Blender" --zip "<absolute ZIP path>" --work-root "<new absolute work directory>" --revision "<candidate commit>"
```

The launcher isolates Blender user resources, installs and enables the ZIP through Blender's
extension command and opens a second process that loads the installed package. It checks the
native architecture and Blender release, then runs the existing hierarchy, material, texture,
rig and animation assertions. Retain the two logs and `result.json` with device evidence.
