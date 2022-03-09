use std::cell::RefCell;
use std::io::Error as IoError;
use std::io::Read;
use std::io::Write;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::Mutex;

use pyo3::exceptions::PyOSError;
use pyo3::exceptions::PyTypeError;
use pyo3::gc::PyGCProtocol;
use pyo3::gc::PyTraverseError;
use pyo3::gc::PyVisit;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyString;
use pyo3::AsPyPointer;
use pyo3::PyDowncastError;
use pyo3::PyNativeType;
use pyo3::PyObject;

// ---------------------------------------------------------------------------

#[macro_export]
macro_rules! transmute_file_error {
    ($self:ident, $e:ident, $msg:expr, $py:expr) => {{
        // Attempt to transmute the Python OSError to an actual
        // Rust `std::io::Error` using `from_raw_os_error`.
        if $e.is_instance::<PyOSError>($py) {
            if let Ok(code) = &$e.pvalue($py).getattr("errno") {
                if let Ok(n) = code.extract::<i32>() {
                    return Err(IoError::from_raw_os_error(n));
                }
            }
        }

        // if the conversion is not possible for any reason we fail
        // silently, wrapping the Python error, and returning a
        // generic Rust error instead.
        $e.restore($py);
        Err(IoError::new(std::io::ErrorKind::Other, $msg))
    }};
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum FileMode {
    Binary,
    Text,
}

// ---------------------------------------------------------------------------

/// A wrapper around a readable Python file borrowed within a GIL lifetime.
#[derive(Debug, Clone)]
pub struct PyFileRead<'p> {
    file: &'p PyAny,
    mode: FileMode,
    buffer: Vec<u8>,  // used only in Text mode
}

impl<'p> PyFileRead<'p> {
    pub fn from_ref(file: &'p PyAny) -> PyResult<PyFileRead<'p>> {
        let res = file.call_method1("read", (0,))?;
        if res.cast_as::<PyBytes>().is_ok() {
            Ok(PyFileRead {
                file,
                mode: FileMode::Binary,
                buffer: Vec::new(),
            })
        } else if res.cast_as::<PyString>().is_ok() {
            Ok(PyFileRead {
                file,
                mode: FileMode::Text,
                buffer: Vec::new(),
            })
        } else {
            let ty = res.get_type().name()?.to_string();
            Err(PyTypeError::new_err(format!(
                "expected bytes, found {}",
                ty
            )))
        }
    }
}

impl<'p> Read for PyFileRead<'p> {
    fn read(&mut self, mut buf: &mut [u8]) -> Result<usize, IoError> {
        match self.mode {
            // In binary mode, just read at most `buf.len()` bytes.
            FileMode::Binary => match self.file.call_method1("read", (buf.len(),)) {
                Ok(obj) => {
                    // Check `fh.read` returned bytes, else raise a `TypeError`.
                    if let Ok(bytes) = obj.extract::<&PyBytes>() {
                        let b = bytes.as_bytes();
                        (&mut buf[..b.len()]).copy_from_slice(b);
                        Ok(b.len())
                    } else {
                        let ty = obj.get_type().name()?.to_string();
                        let msg = format!("expected bytes, found {}", ty);
                        PyTypeError::new_err(msg).restore(self.file.py());
                        Err(IoError::new(
                            std::io::ErrorKind::Other,
                            "read method did not return bytes",
                        ))
                    }
                }
                Err(e) => {
                    transmute_file_error!(self, e, "read method failed", self.file.py())
                }
            }
            // In text mode, we may be given more than `buf.len()` bytes if
            // we request `buf.len()` characters, in that case we store the
            // additional bytes into `self.buffer`
            FileMode::Text => {
                // number of bytes returned
                let mut n = self.buffer.len();
                // copy buffer data
                buf[..n].copy_from_slice(&self.buffer);
                buf = &mut buf[n..];
                self.buffer.clear();
                // read next chunk
                match self.file.call_method1("read", (buf.len(),)) {
                    Ok(obj) => {
                        if let Ok(string) = obj.extract::<&PyString>() {
                            // get raw bytes from the Python string
                            let s = string.to_str()?;
                            let b = s.as_bytes();
                            // copy bytes, if needed cache extra bytes
                            if b.len() <= buf.len() {
                                buf[..b.len()].copy_from_slice(&b);
                                n += b.len();
                            } else {
                                buf.copy_from_slice(&b[..buf.len()]);
                                self.buffer.extend_from_slice(&b[buf.len()..]);
                                n += buf.len();
                            }
                            Ok(n)
                        } else {
                            let ty = obj.get_type().name()?.to_string();
                            let msg = format!("expected str, found {}", ty);
                            PyTypeError::new_err(msg).restore(self.file.py());
                            Err(IoError::new(
                                std::io::ErrorKind::Other,
                                "read method did not return str",
                            ))
                        }
                    }
                    Err(e) => {
                        transmute_file_error!(self, e, "read method failed", self.file.py())
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------

/// A wrapper around a writable Python file borrowed within a GIL lifetime.
pub struct PyFileWrite<'p> {
    file: &'p PyAny,
}

impl<'p> PyFileWrite<'p> {
    pub fn from_ref(file: &'p PyAny) -> PyResult<PyFileWrite<'p>> {
        file.call_method1("write", (PyBytes::new(file.py(), b""),))
            .map(|_| PyFileWrite { file })
    }
}

impl<'p> Write for PyFileWrite<'p> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> {
        let bytes = PyBytes::new(self.file.py(), buf);
        match self.file.call_method1("write", (bytes,)) {
            Ok(obj) => {
                // Check `fh.write` returned int, else raise a `TypeError`.
                if let Ok(len) = usize::extract(&obj) {
                    Ok(len)
                } else {
                    let ty = obj.get_type().name()?.to_string();
                    let msg = format!("expected int, found {}", ty);
                    PyTypeError::new_err(msg).restore(self.file.py());
                    Err(IoError::new(
                        std::io::ErrorKind::Other,
                        "write method did not return int",
                    ))
                }
            }
            Err(e) => {
                transmute_file_error!(self, e, "write method failed", self.file.py())
            }
        }
    }

    fn flush(&mut self) -> Result<(), IoError> {
        match self.file.call_method0("flush") {
            Ok(_) => Ok(()),
            Err(e) => {
                transmute_file_error!(self, e, "flush method failed", self.file.py())
            }
        }
    }
}

// ---------------------------------------------------------------------------

/// A wrapper for a Python file that can outlive the GIL.
pub struct PyFileGILRead {
    file: Mutex<PyObject>,
    mode: FileMode
}

impl PyFileGILRead {
    pub fn from_ref(file: &PyAny) -> PyResult<PyFileGILRead> {
        let res = file.call_method1("read", (0,))?;
        if res.cast_as::<PyBytes>().is_ok() {
            Ok(PyFileGILRead {
                file: Mutex::new(file.to_object(file.py())),
                mode: FileMode::Binary
            })
        } else if res.cast_as::<PyString>().is_ok() {
            Ok(PyFileGILRead {
                file: Mutex::new(file.to_object(file.py())),
                mode: FileMode::Text
            })
        } else {
            let ty = res.get_type().name()?.to_string();
            Err(PyTypeError::new_err(format!(
                "expected bytes, found {}",
                ty
            )))
        }
    }

    pub fn file(&self) -> &Mutex<PyObject> {
        &self.file
    }
}

impl Read for PyFileGILRead {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        let gil = Python::acquire_gil();
        let py = gil.python();
        let file = self.file.lock().unwrap();
        match file.to_object(py).call_method1(py, "read", (buf.len(),)) {
            Ok(obj) => {
                // Check `fh.read` returned bytes, else raise a `TypeError`.
                if let Ok(bytes) = obj.cast_as::<PyBytes>(py) {
                    let b = bytes.as_bytes();
                    (&mut buf[..b.len()]).copy_from_slice(b);
                    Ok(b.len())
                } else {
                    let ty = obj.as_ref(py).get_type().name()?.to_string();
                    let msg = format!("expected bytes, found {}", ty);
                    PyTypeError::new_err(msg).restore(py);
                    Err(IoError::new(
                        std::io::ErrorKind::Other,
                        "fh.read did not return bytes",
                    ))
                }
            }
            Err(e) => transmute_file_error!(self, e, "read method failed", py),
        }
    }
}
