use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::io::Read;
use std::io::Write;

use pyo3::exceptions::PyOSError;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyString;
use pyo3::types::PyType;
use pyo3::FromPyPointer;
use pyo3::PyObject;

// ---------------------------------------------------------------------------

#[macro_export]
macro_rules! transmute_file_error {
    ($self:ident, $e:ident, $msg:expr, $py:expr) => {{
        // Attempt to transmute the Python OSError to an actual
        // Rust `std::io::Error` using `from_raw_os_error`.
        if $e.is_instance($py, PyType::new::<PyOSError>($py)) {
            if let Ok(code) = &$e.value($py).getattr("errno") {
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

// -----------------------------------------------------------------------------

/// A wrapper around a readable Python file borrowed within a GIL lifetime.
#[derive(Debug, Clone)]
pub enum PyFileRead<'p> {
    Binary(PyFileReadBin<'p>),
    Text(PyFileReadText<'p>),
}

impl<'p> PyFileRead<'p> {
    pub fn from_ref(file: &'p PyAny) -> PyResult<Self> {
        let res = file.call_method1("read", (0,))?;
        if res.cast_as::<PyBytes>().is_ok() {
            PyFileReadBin::new(file).map(Self::Binary)
        } else if res.cast_as::<PyString>().is_ok() {
            PyFileReadText::new(file).map(Self::Text)
        } else {
            let ty = res.get_type().name()?.to_string();
            Err(PyTypeError::new_err(format!(
                "expected bytes or str, found {}",
                ty
            )))
        }
    }
}

impl<'p> Read for PyFileRead<'p> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        match self {
            PyFileRead::Binary(readbin) => readbin.read(buf),
            PyFileRead::Text(readtext) => readtext.read(buf),
        }
    }
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PyFileReadBin<'p> {
    file: &'p PyAny,
    readinto: bool,
}

impl<'p> PyFileReadBin<'p> {
    pub fn new(file: &'p PyAny) -> PyResult<Self> {
        #[cfg(feature = "cpython")]
        {
            file.hasattr("readinto")
                .map(|readinto| Self { file, readinto })
        }
        #[cfg(not(feature = "cpython"))]
        {
            Ok(Self {
                file,
                readinto: false,
            })
        }
    }
}

impl<'p> Read for PyFileReadBin<'p> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        // Try to use the zero-copy method if possible
        if self.readinto {
            // prepare a `memoryview` to expose the buffer
            let memoryview = unsafe {
                PyAny::from_owned_ptr(
                    self.file.py(),
                    pyo3::ffi::PyMemoryView_FromMemory(
                        buf.as_mut_ptr() as *mut libc::c_char,
                        buf.len() as isize,
                        pyo3::ffi::PyBUF_WRITE,
                    ),
                )
            };
            // read directly into the `memoryview`
            match self.file.call_method1("readinto", (memoryview,)) {
                Ok(obj) => {
                    if let Ok(n) = obj.extract::<usize>() {
                        Ok(n)
                    } else {
                        let ty = obj.get_type().name()?.to_string();
                        let msg = format!("expected int, found {}", ty);
                        PyTypeError::new_err(msg).restore(self.file.py());
                        Err(IoError::new(
                            std::io::ErrorKind::Other,
                            "readinto method did not return int",
                        ))
                    }
                }
                Err(e) => {
                    transmute_file_error!(self, e, "readinto method failed", self.file.py())
                }
            }
        } else {
            match self.file.call_method1("read", (buf.len(),)) {
                Ok(obj) => {
                    // Check `fh.read` returned bytes, else raise a `TypeError`.
                    if let Ok(bytes) = obj.extract::<&PyBytes>() {
                        let b = bytes.as_bytes();
                        buf[..b.len()].copy_from_slice(b);
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
        }
    }
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PyFileReadText<'p> {
    file: &'p PyAny,
    buffer: Vec<u8>,
}

impl<'p> PyFileReadText<'p> {
    pub fn new(file: &'p PyAny) -> PyResult<Self> {
        Ok(Self {
            file,
            buffer: Vec::new(),
        })
    }
}

impl<'p> Read for PyFileReadText<'p> {
    fn read(&mut self, mut buf: &mut [u8]) -> Result<usize, IoError> {
        // number of bytes returned
        let mut n = self.buffer.len();
        // copy buffer data from previous call
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
                        buf[..b.len()].copy_from_slice(b);
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

// ---------------------------------------------------------------------------

/// A wrapper for a Python file that can outlive the GIL.
pub enum PyFileGILRead {
    Binary(PyFileGILReadBin),
    Text(PyFileGILReadText),
}

impl PyFileGILRead {
    pub fn from_ref(file: &PyAny) -> PyResult<PyFileGILRead> {
        let py = file.py();
        let res = file.call_method1("read", (0,))?;
        if res.cast_as::<PyBytes>().is_ok() {
            let obj = file.into_py(py);
            PyFileGILReadBin::new(py, obj).map(Self::Binary)
        } else if res.cast_as::<PyString>().is_ok() {
            let obj = file.into_py(py);
            PyFileGILReadText::new(py, obj).map(Self::Text)
        } else {
            let ty = res.get_type().name()?.to_string();
            Err(PyTypeError::new_err(format!(
                "expected bytes or str, found {}",
                ty
            )))
        }
    }
}

impl Read for PyFileGILRead {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        match self {
            PyFileGILRead::Binary(readbin) => readbin.read(buf),
            PyFileGILRead::Text(readtext) => readtext.read(buf),
        }
    }
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PyFileGILReadBin {
    file: PyObject,
    readinto: bool,
}

impl PyFileGILReadBin {
    pub fn new(py: Python, file: PyObject) -> PyResult<Self> {
        #[cfg(feature = "cpython")]
        {
            file.as_ref(py)
                .hasattr("readinto")
                .map(|readinto| Self { file, readinto })
        }
        #[cfg(not(feature = "cpython"))]
        {
            Ok(Self {
                file,
                readinto: false,
            })
        }
    }
}

impl Read for PyFileGILReadBin {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        // acquire a GIL
        let gil = Python::acquire_gil();
        let py = gil.python();
        // emulate a PyFileRead
        let reference = self.file.as_ref(py);
        let mut reader = PyFileReadBin {
            file: reference,
            readinto: self.readinto,
        };
        reader.read(buf)
    }
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PyFileGILReadText {
    file: PyObject,
    buffer: Vec<u8>,
}

impl PyFileGILReadText {
    pub fn new(_py: Python, file: PyObject) -> PyResult<Self> {
        Ok(Self {
            file,
            buffer: Vec::new(),
        })
    }
}

impl Read for PyFileGILReadText {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        // acquire a GIL
        let gil = Python::acquire_gil();
        let py = gil.python();
        // emulate a PyFileRead
        let reference = self.file.as_ref(py);
        let mut reader = PyFileReadText {
            file: reference,
            buffer: std::mem::take(&mut self.buffer),
        };
        // read and store the number of bytes read
        let result = reader.read(buf);
        // swap back the buffer and return result
        std::mem::swap(&mut reader.buffer, &mut self.buffer);
        result
    }
}

// ---------------------------------------------------------------------------

/// A wrapper around a writable Python file borrowed within a GIL lifetime.
#[derive(Debug, Clone)]
pub enum PyFileWrite<'p> {
    Binary(PyFileWriteBin<'p>),
    Text(PyFileWriteText<'p>),
}

impl<'p> PyFileWrite<'p> {
    pub fn from_ref(file: &'p PyAny) -> PyResult<Self> {
        // try writing bytes
        let bytes = PyBytes::new(file.py(), b"");
        if file.call_method1("write", (bytes,)).is_ok() {
            return PyFileWriteBin::new(file).map(Self::Binary);
        };
        // try writing strings
        let s = PyString::new(file.py(), "");
        match file.call_method1("write", (s,)) {
            Ok(_) => PyFileWriteText::new(file).map(Self::Text),
            Err(e) => Err(e),
        }
    }
}

impl<'p> Write for PyFileWrite<'p> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> {
        match self {
            PyFileWrite::Binary(writebin) => writebin.write(buf),
            PyFileWrite::Text(writetext) => writetext.write(buf),
        }
    }

    fn flush(&mut self) -> Result<(), IoError> {
        match self {
            PyFileWrite::Binary(writebin) => writebin.flush(),
            PyFileWrite::Text(writetext) => writetext.flush(),
        }
    }
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PyFileWriteBin<'p> {
    file: &'p PyAny,
}

impl<'p> PyFileWriteBin<'p> {
    pub fn new(file: &'p PyAny) -> PyResult<Self> {
        Ok(Self { file })
    }
}

impl<'p> Write for PyFileWriteBin<'p> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> {
        // FIXME(@althonos): This is copying the buffer data into the bytes
        //                   first, ideally we could just pass a `memoryview`
        let bytes = PyBytes::new(self.file.py(), buf);
        match self.file.call_method1("write", (bytes,)) {
            Ok(obj) => {
                // Check `fh.write` returned int, else raise a `TypeError`.
                if let Ok(len) = usize::extract(obj) {
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

#[derive(Debug, Clone)]
pub struct PyFileWriteText<'p> {
    file: &'p PyAny,
}

impl<'p> PyFileWriteText<'p> {
    pub fn new(file: &'p PyAny) -> PyResult<Self> {
        Ok(Self { file })
    }
}

impl<'p> Write for PyFileWriteText<'p> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> {
        // FIXME(@althonos): This will fail in the event the buffer does not
        //                   contain valid UTF-8, which may be the case if
        //                   the last character is not a complete code point.
        //                   In that case, we should instead write as much as
        //                   possible instead of failing.
        let decoded = match std::str::from_utf8(buf) {
            Ok(s) => s,
            Err(e) => return Err(IoError::new(IoErrorKind::InvalidData, e)), // Err(e) => return Err(PyUnicodeError::new_err(e.to_string())),
        };
        let s = PyString::new(self.file.py(), decoded);
        match self.file.call_method1("write", (s,)) {
            Ok(obj) => Ok(buf.len()), // FIXME?
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
