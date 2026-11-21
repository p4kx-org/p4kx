### Fixed

- Retire outdated source receipts when ordinary extraction or folder conversion replaces a file, including replacements with identical bytes.
- Create new ordinary files atomically without a lock when the destination and its receipts are absent.
