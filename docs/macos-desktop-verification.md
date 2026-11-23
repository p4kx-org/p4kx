# macOS desktop verification

These desktop checks are optional supplements to the [hosted native macOS checks](macos-support.md).
Use Apple Silicon and the exact candidate app and extension ZIP for any observations you report.
Record the OS version, architecture, source commit, binary and artifact hashes, and outcomes
separately from hosted synthetic CI evidence.

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
- Run the shared procedure using CoreAudio and the installed app.
  [Desktop audio verification](supported-platforms.md#desktop-audio-verification)

## Installed Blender extension

For an optional installed-extension check, use native Blender 5.2 LTS and the exact release ZIP.
The work directory must be new.

```text
python3 blender_addon/tests/native_extension_smoke.py --blender "/Applications/Blender.app/Contents/MacOS/Blender" --zip "<absolute ZIP path>" --work-root "<new absolute work directory>" --revision "<candidate commit>"
```

The launcher isolates Blender user resources, installs and enables the ZIP through Blender's
extension command and opens a second process that loads the installed package. It checks the
native architecture and Blender release, then runs the existing hierarchy, material, texture,
rig and animation assertions. Retain the two logs and `result.json`. Record the associated
source commit, OS version and hash of the attested `release-addon` asset.
