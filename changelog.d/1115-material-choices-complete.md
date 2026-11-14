### Fixed

- Material choices retain readable descendants and siblings beyond fixed projection depth and
  node-count cutoffs. Selection, copying, comparison, diagnostics and cleanup traverse complete
  owned trees without recursive thread-stack growth.
