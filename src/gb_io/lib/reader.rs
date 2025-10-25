use std::fs::File;
use std::io::Error as IoError;
use std::io::Read;
use std::ops::DerefMut;
use std::path::Path;
use std::path::PathBuf;

use gb_io::reader::SeqReader;

use pyo3::exceptions::PyOSError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use super::pyfile::PyFileGILRead;
use super::Convert;
use super::PyInterner;
use super::Record;

// ---------------------------------------------------------------------------

/// An enum providing `Read` for either Python file-handles or filesystem files.
pub enum Handle {
    FsFile(File),
    PyFile(PyFileGILRead),
}

impl TryFrom<PathBuf> for Handle {
    type Error = std::io::Error;
    fn try_from(p: PathBuf) -> Result<Self, Self::Error> {
        let file = File::open(&p)?;
        Ok(Handle::FsFile(file))
    }
}

impl Read for Handle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        match self {
            Handle::FsFile(f) => f.read(buf),
            Handle::PyFile(f) => f.read(buf),
        }
    }
}

// ---------------------------------------------------------------------------

/// An iterator over the `~gb_io.Record` contained in a file.
#[pyclass(module = "gb_io")]
pub struct RecordReader {
    reader: SeqReader<Handle>,
    interner: PyInterner,
}

impl RecordReader {
    fn new(reader: SeqReader<Handle>) -> PyResult<Self> {
        Ok(Self {
            reader,
            interner: Default::default(),
        })
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> PyResult<Self> {
        let p = path.as_ref();
        match Handle::try_from(p.to_owned()) {
            Ok(handle) => Self::new(SeqReader::new(handle)),
            Err(e) => {
                if let Some(code) = e.raw_os_error() {
                    Err(PyOSError::new_err((code, e.to_string())))
                } else {
                    Err(PyOSError::new_err(e.to_string()))
                }
            }
        }
    }

    pub fn from_handle(obj: Bound<PyAny>) -> PyResult<Self> {
        match PyFileGILRead::from_ref(obj).map(Handle::PyFile) {
            Ok(handle) => Self::new(SeqReader::new(handle)),
            Err(e) => Err(e),
        }
    }
}

#[pymethods]
impl RecordReader {
    fn __iter__<'py>(slf: PyRefMut<'py, Self>) -> PyResult<PyRefMut<'py, Self>> {
        Ok(slf)
    }

    fn __next__<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<Option<Bound<'py, Record>>> {
        let py = slf.py();
        let slf = slf.deref_mut();
        let interner = &mut slf.interner;
        match py.detach(|| slf.reader.next()) {
            None => Ok(None),
            Some(Ok(seq)) => Ok(Some(seq.convert_bound_with(py, interner)?)),
            Some(Err(e)) => {
                Python::attach(|py| {
                    if PyErr::occurred(py) {
                        Err(PyErr::fetch(py))
                    } else {
                        // FIXME: error management
                        let msg = format!("parser failed: {}", e);
                        Err(PyRuntimeError::new_err(msg))
                    }
                })
            }
        }
    }
}
