use std::fs::File;
use std::io::Error as IoError;
use std::io::Read;
use std::ops::DerefMut;
use std::path::Path;
use std::path::PathBuf;

use gb_io::reader::SeqReader;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;


use super::pyfile::PyFileGILRead;
use super::Record;

// ---------------------------------------------------------------------------

/// An enum providing `Read` for either Python file-handles or filesystem files.
pub enum Handle {
    FsFile(File, PathBuf),
    PyFile(PyFileGILRead),
}

// impl Handle {
//     fn handle(&self) -> PyObject {
//         let gil = Python::acquire_gil();
//         let py = gil.python();
//         match self {
//             Handle::FsFile(_, path) => path.display().to_string().to_object(py),
//             Handle::PyFile(f) => f.file().lock().unwrap().to_object(py),
//         }
//     }
// }

impl TryFrom<PathBuf> for Handle {
    type Error = std::io::Error;
    fn try_from(p: PathBuf) -> Result<Self, Self::Error> {
        let file = File::open(&p)?;
        Ok(Handle::FsFile(file, p))
    }
}

impl Read for Handle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        match self {
            Handle::FsFile(f, _) => f.read(buf),
            Handle::PyFile(f) => f.read(buf),
        }
    }
}

// ---------------------------------------------------------------------------

/// An iterator over the `~gb_io.Record` contained in a file.
#[pyclass(module = "gb_io")]
pub struct RecordReader {
    reader: SeqReader<Handle>,
}

impl RecordReader {
    fn new(reader: SeqReader<Handle>) -> PyResult<Self> {
        Ok(Self { reader })
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> PyResult<Self> {
        let p = path.as_ref();
        match Handle::try_from(p.to_owned()) {
            Ok(handle) => Self::new(SeqReader::new(handle)),
            Err(_e) => unimplemented!("error management"),
        }
    }

    pub fn from_handle(obj: &PyAny) -> PyResult<Self> {
        match PyFileGILRead::from_ref(obj).map(Handle::PyFile) {
            Ok(handle) => Self::new(SeqReader::new(handle)),
            Err(e) => Err(e),
        }
    }
}

// #[pyproto]
// impl PyObjectProtocol for RecordReader {
//     fn __repr__(&self) -> PyResult<PyObject> {
//         let gil = Python::acquire_gil();
//         let py = gil.python();
//         let fmt = PyString::new(py, "gb_io.RecordReader({!r})").to_object(py);
//         fmt.call_method1(py, "format", (&self.reader.as_ref().get_ref().handle(),))
//     }
// }

#[pymethods]
impl RecordReader {
    fn __iter__<'p>(slf: PyRefMut<'p, Self>) -> PyResult<PyRefMut<'p, Self>> {
        Ok(slf)
    }

    fn __next__<'p>(mut slf: PyRefMut<'p, Self>) -> PyResult<Option<Record>> {
        match slf.deref_mut().reader.next() {
            None => Ok(None),
            Some(Ok(seq)) => Ok(Some(Record::from(seq))),
            Some(Err(e)) => {
                let gil = Python::acquire_gil();
                let py = gil.python();
                if PyErr::occurred(py) {
                    Err(PyErr::fetch(py))
                } else {
                    // FIXME: error management
                    let msg = format!("parser failed: {}", e);
                    Err(PyRuntimeError::new_err(msg))
                }
            }
        }
    }
}
