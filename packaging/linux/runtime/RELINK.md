# AppImage runtime source and relinking

This directory contains the complete corresponding source for the embedded AppImage runtime,
its static library source archives, original licence files, distribution build recipes, patches,
and the exact signed APK inputs. `inputs.json` and `runtime-inventory.json` record source commits,
versions and hashes. The APKs contain the original library archives and headers; their
corresponding sources and Alpine recipes are alongside them.

`type2-runtime/src/runtime/Makefile` contains the complete compiler and static linker command.
The runtime source is MIT-licensed. Each library's original notices are under `licences/`;
libfuse's LGPL terms remain applicable to that library.

To reconstruct the supplied build context, copy `type2-runtime` to a directory named `upstream`
alongside `Dockerfile`, `collect-sources.py`, `inputs.json` and `packages`. Build with the
`builder_image` and `source_commit` recorded in `inputs.json` as the `BUILDER_IMAGE` and
`RUNTIME_COMMIT` Docker build arguments. Use a native CPU matching the supplied package.
The build scripts and source archives document every installed static library.

To use a modified library, unpack its source archive, apply the recorded distribution patches
when applicable, and follow the supplied build recipe to produce its static archive and headers.
The `toolchain` Docker stage provides the build environment. Invoke the runtime Makefile there
with `LIBS` beginning with `-L` for the modified archive directory, followed by the original
library list; preserve the original `CFLAGS` and append modified include paths when necessary.
This relinks the MIT runtime source against the modified library without rebuilding the P4KX
applications. Run the supplied `scripts/build-runtime.sh` to retain AppImage magic bytes and
debug-link handling.

To replace the embedded runtime, supply the resulting native runtime to appimagetool's
`--runtime-file` option when repackaging the P4KX AppDir. The payload's three native application
executables are independent of this relinking operation.
