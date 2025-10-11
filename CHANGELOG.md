# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).


## [Unreleased]
[Unreleased]: https://github.com/althonos/gb-io.py/compare/v0.3.7...HEAD

## [v0.3.7] - 2025-10-11
[v0.3.7]: https://github.com/althonos/gb-io.py/compare/v0.3.6...v0.3.7

### Changed
- Bump `pyo3` dependency to `v0.24` for Python 3.14 compatibility ([#64](https://github.com/althonos/gb-io.py/pull/64), by [@refack](https://github.com/refack)).
- Build wheels in Limited API for the Python 3.7 ABI.


## [v0.3.6] - 2025-05-05
[v0.3.6]: https://github.com/althonos/gb-io.py/compare/v0.3.5...v0.3.6

### Changed
- Bump `gb-io` dependency to `v0.9.0`.


## [v0.3.5] - 2025-03-18
[v0.3.5]: https://github.com/althonos/gb-io.py/compare/v0.3.4...v0.3.5

### Changed
- Bump `gb-io` dependency to `v0.8.0`.


## [v0.3.4] - 2025-01-22
[v0.3.4]: https://github.com/althonos/gb-io.py/compare/v0.3.3...v0.3.4

### Added
- Support for Python 3.13.

### Changed
- Bump `pyo3` dependency to `v0.23` and remove deprecated interface.
- Bump `pyo3-built` dependency to `v0.6.0`.
- Use `maturin` instead of `setuptools-rust` to build crate.
- Use PyData Sphinx theme to render the documentation.

### Fixed
- Change working directory to access test data in `tests.test_doctest`.


## [v0.3.3] - 2024-04-14
[v0.3.3]: https://github.com/althonos/gb-io.py/compare/v0.3.2...v0.3.3

### Changed
- Bump `pyo3` dependency to `v0.21`.


## [v0.3.2] - 2024-04-01
[v0.3.2]: https://github.com/althonos/gb-io.py/compare/v0.3.1...v0.3.2

### Fixed
- `gb_io.dump` not extracting qualifiers from Python-created records ([#42](https://github.com/althonos/gb-io.py/issues/42)).
- `Qualifier.__repr__` and various `__repr__` of `Location` subclasses not using `repr`-formatting.


## [v0.3.1] - 2024-03-28
[v0.3.1]: https://github.com/althonos/gb-io.py/compare/v0.3.0...v0.3.1

### Fixed
- `Complement.strand` not extracting the right attribute from the inner `Location`.


## [v0.3.0] - 2024-03-25
[v0.3.0]: https://github.com/althonos/gb-io.py/compare/v0.2.1...v0.3.0

### Added
- Python constructors to all types.
- Properties with getter and setters for all remaining `Record` fields.
- Documentation with API reference at https://gb-io.readthedocs.io.

### Changed
- Bump `pyo3` dependency to `v0.20`.
- Add wheels for Python 3.11 and 3.12.
- Reorganize code to facilitate object creation.
- Implement copy-on-access for `Record` and `Feature` attributes.
- `strand` property to some common `Location` types.
- Make `Record.sequence` a `bytearray` to allow changing the sequence content.


## [v0.2.1] - 2022-12-16
[v0.2.1]: https://github.com/althonos/gb-io.py/compare/v0.2.0...v0.2.1

### Added
- `source` and `organism` properties to `Record` objects.
- Support for Python 3.11.

### Changed
- Bumped `pyo3` dependency to `v0.17.3`.

### Removed
- Support for Python 3.6.


## [v0.2.0] - 2022-08-16
[v0.2.0]: https://github.com/althonos/gb-io.py/compare/v0.1.2...v0.2.0

### Added
- `gb_io.dump` method to write one or more `Record` objects to a file.

### Fixed
- Compilation issues with modern `setuptools-rust` versions.
- Avoid using `readinto` method of file-like objects when compiling for PyPy because of compatibility issues with passing `memoryview` arguments.


## [v0.1.2] - 2022-05-12
[v0.1.2]: https://github.com/althonos/gb-io.py/compare/v0.1.1...v0.1.2

### Added
- Extraction of `Join`, `Order`, `Bond` and `OneOf` feature locations.
- `start` and `end` properties for `Complement` and `Between`.


## [v0.1.1] - 2022-04-01
[v0.1.1]: https://github.com/althonos/gb-io.py/compare/v0.1.0...v0.1.1

### Added
- Extraction of `Range`, `Complement` and `Between` locations for a feature.

### Fixed
- Exception chaining for file-like objects.
- Management of OS errors raised on the Rust side.


## [v0.1.0] - 2022-03-10
[v0.1.0]: https://github.com/althonos/gb-io.py/compare/e092b0369...v0.1.0

Initial release.
