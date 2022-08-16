# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).


## [Unreleased]
[Unreleased]: https://github.com/althonos/gb-io.py/compare/v0.2.0...HEAD


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
