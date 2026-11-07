### Fixed

- Archive-key demand counts identify physical Entries across complete nested container chains,
  including duplicate paths. An Entry encountered without a key and with a wrong key contributes
  once to the distinct unread total while retaining both failure causes.
