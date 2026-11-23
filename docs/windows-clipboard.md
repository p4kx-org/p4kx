# Native Windows text clipboard

The GUI enables `eframe/clipboard` in the shared `cfg(any(unix, windows))` dependency table.
Windows therefore uses egui-winit's arboard backend for existing copy and paste actions.
The Unix table still covers Linux and macOS; field behavior and the P4KX design are unchanged.

The resolved dependency versions are recorded in `Cargo.lock`. The feature uses arboard and
its clipboard-win Windows dependency already present in that lockfile. Licence generation must
use the final feature graph and every native release target.

## Checks

The focused `the_system_clipboard_feature_is_enabled_for_windows_and_unix` test checks the
dependency declaration. The feature graph can also be inspected with:

```powershell
cargo tree --locked -p p4kx-gui -e features --target x86_64-pc-windows-msvc
cargo tree --locked -p p4kx-gui -e features --target aarch64-pc-windows-msvc
```

A successful declaration or in-process egui string test does not establish system clipboard
behavior. Run a source-built GUI and a separate standard Windows editor such as Notepad,
using synthetic text and an isolated profile.

Set an absolute `P4KX_FOLDERS_ROOT` before launching the GUI and remove inherited
`P4KX_ARCHIVE_KEY` and `P4KX_KEY_FILE`. Its `config`, `cache` and `data` subfolders isolate the
actual application paths, including on Windows; see [profile folders](profile-folders.md).

| Case | Operation | Required observation |
| --- | --- | --- |
| Unicode | Copy a synthetic P4KX text preview containing accents, combining characters, CJK and an emoji, then paste into Notepad. | Every character survives. |
| Multiple lines | Copy a multiline preview into Notepad; copy multiline editor text into the existing exclusion-Filter text area of an Extraction dialog. | Logical lines and tabs survive; no unexpected truncation. |
| Editor to GUI | Copy from Notepad and paste into an existing editable P4KX field. | The actual OS clipboard text appears, without a prior P4KX copy. |
| Empty text | Copy an empty preview and verify the external clipboard; clear clipboard text and paste into a populated P4KX field. | No stale text is inserted and no error or hang occurs. |
| Busy clipboard | Have a separate helper hold the Windows clipboard briefly while asking P4KX to copy or paste. | The window remains responsive and recovers when the clipboard is released. |

Cancel the Extraction dialog after testing its multiline field. There is no need to write an
export. Record the OS/CPU, source revision/worktree identity, GUI executable digest, editor,
actual observations and any inconclusive case on the owner's Windows AMD64 desktop. Windows
ARM64 retains its hosted native runtime and package checks.

## Windows AMD64 dry-run evidence

The packaged GUI from [Release dry run 45173014290](https://github.com/p4kx-org/p4kx/actions/runs/45173014290)
ran on Windows 11 build 26300 on the maintainer's AMD64 desktop. This dry run reports version
0.4.2 and source revision `f352d03578e22d98d4e7cd5d0ae29e61223f3418`. The executable SHA-256 is
`64C019B89253287FDD5D96ED344D53A9D71D7F731157145204663625D3DA1964`.

- P4KX copied the synthetic UTF-8 preview to Notepad. The accent, combining mark, CJK,
  emoji and tab survived. Notepad converted LF to CRLF on save.
- P4KX's multiline Query editor received the Notepad text and normalized CRLF to LF. Copying the empty preview left the prior clipboard value in place.
  Pasting from an empty clipboard left the Query text intact.
- A separate native window held the clipboard open during a paste attempt. The text
  stayed intact, and subsequent typing reached the Query editor while the hold was
  active. The helper released its handle and the clipboard sequence number stayed
  constant throughout the hold.

## Failure and empty-text semantics

The pinned arboard Windows implementation retries clipboard acquisition a bounded number of
times. egui-winit logs copy/paste backend errors and returns without an application panic or
unbounded retry. A missing OS backend can use egui-winit's in-process fallback; a busy arboard
operation is an error, not proof that data reached another program.

Explicit copy dispatches its text, including an empty string, to the backend. The pinned paste
handler normalizes CRLF to LF and ignores an empty string. Existing single-line and multiline
field rules still apply.

## Sources

- [eframe 0.37.0 clipboard feature](https://github.com/emilk/egui/blob/0.37.0/crates/eframe/Cargo.toml)
- [egui-winit 0.37.0 clipboard backend](https://github.com/emilk/egui/blob/0.37.0/crates/egui-winit/src/clipboard.rs)
- [egui-winit 0.37.0 input/output handling](https://github.com/emilk/egui/blob/0.37.0/crates/egui-winit/src/lib.rs)
- [arboard 3.7.0 Windows acquisition](https://github.com/1Password/arboard/blob/v3.7.0/src/platform/windows.rs)
