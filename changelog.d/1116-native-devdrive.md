### Fixed

- CI: use the ordinary Cargo target directory on Windows ARM64, where the optional ReFS
  optimization's Hyper-V cleanup commands are unavailable. Native builds and package checks remain strict.