# Contributing to `gb-io.py`

For bug fixes or new features, please file an issue before submitting a
pull request. If the change isn't trivial, it may be best to wait for
feedback.

## Running tests

Tests are written as usual Python unit tests with the `unittest` module of
the standard library. Running them requires the extension to be built locally:

```console
$ python setup.py build_ext --inplace
$ python -m unittest discover -vv
```

## Coding guidelines

This project targets all Python versions supported by the latest release of
PyO3. It should be able to compile with the `stable` version of the Rust
compiler.

### Docstrings

The docstring lines should not be longer than 76 characters (which allows
rendering the entire module in a 80x24 terminal window without soft-wrap).  
Docstrings should be written in Google format.

### Format

Make sure to format the code with `cargo fmt` before making a commit. This can
be done automatically with a pre-commit hook.
