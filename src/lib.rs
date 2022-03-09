extern crate gb_io;
extern crate libc;
extern crate pyo3;
extern crate pyo3_built;

mod built;
mod iter;
mod pyfile;

use std::io::Read;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::RwLock;

use gb_io::reader::SeqReader;
use gb_io::seq::Seq;
use gb_io::seq::Topology;
use gb_io::QualifierKey;
use pyo3::exceptions::PyIndexError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyDict;
use pyo3::types::PyList;
use pyo3::types::PyString;
use pyo3_built::pyo3_built;

use self::iter::RecordReader;
use self::pyfile::PyFileRead;

// ---------------------------------------------------------------------------

/// A single GenBank record.
#[pyclass(module = "gb_io")]
#[derive(Debug)]
pub struct Record {
    seq: Arc<RwLock<Seq>>,
}

#[pymethods]
impl Record {
    /// `str`, optional: The name of the record, or `None`.
    #[getter]
    fn get_name(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        let seq = slf.seq.read().expect("cannot read lock");
        match &seq.name {
            None => Ok(slf.py().None()),
            Some(n) => Ok(PyString::new(slf.py(), &n).into_py(slf.py())),
        }
    }

    #[setter]
    fn set_name(slf: PyRefMut<'_, Self>, name: Option<String>) -> PyResult<()> {
        let mut seq = slf.seq.write().expect("cannot write lock");
        seq.name = name;
        Ok(())
    }

    /// `str`: The topology of the record.
    #[getter]
    fn get_topology(slf: PyRef<'_, Self>) -> PyResult<&str> {
        let seq = slf.seq.read().expect("cannot read lock");
        match &seq.topology {
            Topology::Linear => Ok("linear"),
            Topology::Circular => Ok("circular"),
        }
    }

    #[setter]
    fn set_topology(slf: PyRefMut<'_, Self>, topology: &str) -> PyResult<()> {
        let mut seq = slf.seq.write().expect("cannot write lock");
        match topology {
            "linear" => {
                seq.topology = Topology::Linear;
                Ok(())
            }
            "circular" => {
                seq.topology = Topology::Circular;
                Ok(())
            }
            other => {
                let message = format!("invalid topology: {:?}", other);
                Err(PyValueError::new_err(message))
            }
        }
    }

    /// `str`, optional: The definition of the record, or `None`.
    #[getter]
    fn get_definition(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        let seq = slf.seq.read().expect("cannot read lock");
        match &seq.definition {
            None => Ok(slf.py().None()),
            Some(n) => Ok(PyString::new(slf.py(), &n).into_py(slf.py())),
        }
    }

    #[setter]
    fn set_definition(slf: PyRefMut<'_, Self>, definition: Option<String>) -> PyResult<()> {
        let mut seq = slf.seq.write().expect("cannot write lock");
        seq.definition = definition;
        Ok(())
    }

    /// `str`, optional: The accession of the record, or `None`.
    #[getter]
    fn get_accession(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        let seq = slf.seq.read().expect("cannot read lock");
        match &seq.accession {
            None => Ok(slf.py().None()),
            Some(n) => Ok(PyString::new(slf.py(), &n).into_py(slf.py())),
        }
    }

    #[setter]
    fn set_accession(slf: PyRefMut<'_, Self>, accession: Option<String>) -> PyResult<()> {
        let mut seq = slf.seq.write().expect("cannot write lock");
        seq.accession = accession;
        Ok(())
    }

    /// `str`, optional: The version of the record, or `None`.
    #[getter]
    fn get_version(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        let seq = slf.seq.read().expect("cannot read lock");
        match &seq.version {
            None => Ok(slf.py().None()),
            Some(v) => Ok(PyString::new(slf.py(), v).into_py(slf.py())),
        }
    }

    #[setter]
    fn set_version(slf: PyRefMut<'_, Self>, version: Option<String>) -> PyResult<()> {
        let mut seq = slf.seq.write().expect("cannot write lock");
        seq.version = version;
        Ok(())
    }

    // TODO: date, len, molecule_type, division,
    //       source, dblink, keywords,
    //       references, comments, contig, sequence, features

    #[getter]
    fn get_sequence(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        let seq = slf.seq.read().expect("failed to read lock");
        Ok(PyBytes::new(slf.py(), &seq.seq).into())
    }

    #[getter]
    fn get_features(slf: PyRef<'_, Self>) -> PyResult<Py<Features>> {
        Py::new(
            slf.py(),
            Features {
                seq: slf.seq.clone(),
            },
        )
    }
}

impl From<Seq> for Record {
    fn from(seq: Seq) -> Self {
        Self {
            seq: Arc::new(RwLock::new(seq)),
        }
    }
}

// ---------------------------------------------------------------------------

/// A collection of features in a single record.
#[pyclass(module = "gb_io")]
#[derive(Debug)]
pub struct Features {
    seq: Arc<RwLock<Seq>>,
}

#[pymethods]
impl Features {
    fn __len__(slf: PyRef<'_, Self>) -> PyResult<usize> {
        let seq = slf.seq.read().expect("failed to read lock");
        Ok(seq.features.len())
    }

    fn __getitem__(slf: PyRef<'_, Self>, mut item: isize) -> PyResult<Py<Feature>> {
        let seq = slf.seq.read().expect("failed to read lock");
        let length = seq.features.len();
        if item < 0 {
            item += length as isize;
        }
        if item < 0 || item >= length as isize {
            Err(PyIndexError::new_err(item))
        } else {
            Py::new(
                slf.py(),
                Feature {
                    seq: slf.seq.clone(),
                    index: item as usize,
                },
            )
        }
    }
}

#[pyclass(module = "gb_io")]
#[derive(Debug)]
pub struct Feature {
    seq: Arc<RwLock<Seq>>,
    index: usize,
}

#[pymethods]
impl Feature {
    #[getter(type)]
    fn get_ty(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let seq = slf.seq.read().expect("failed to read lock");
        if slf.index < seq.features.len() {
            let ty = seq.features[slf.index].kind.deref();
            Ok(PyString::new(py, ty).into_py(py))
        } else {
            panic!("fuck")
        }
    }

    #[getter]
    fn get_qualifiers<'py>(slf: PyRef<'py, Self>) -> PyResult<Py<Qualifiers>> {
        Py::new(
            slf.py(),
            Qualifiers {
                seq: slf.seq.clone(),
                index: slf.index,
            },
        )
    }
}

#[pyclass(module = "gb_io")]
#[derive(Debug)]
pub struct Qualifiers {
    seq: Arc<RwLock<Seq>>,
    index: usize,
}

#[pymethods]
impl Qualifiers {
    /// Group the qualifiers by key into a dictionary.
    fn to_dict(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        let seq = slf.seq.read().expect("failed to read lock");
        let feature = &seq.features[slf.index];

        let dict = PyDict::new(slf.py());
        for (key, value) in feature.qualifiers.iter() {
            if let Some(v) = value {
                let l = dict
                    .call_method1("setdefault", (key.deref(), PyList::empty(slf.py())))?
                    .cast_as::<PyList>()?;
                l.append(PyString::new(slf.py(), v))?;
            }
        }

        Ok(dict.into_py(slf.py()))
    }

    fn __len__(slf: PyRef<'_, Self>) -> PyResult<usize> {
        let seq = slf.seq.read().expect("failed to read lock");
        let feature = &seq.features[slf.index];
        Ok(feature.qualifiers.len())
    }

    fn __getitem__(slf: PyRef<'_, Self>, mut item: isize) -> PyResult<Py<Qualifier>> {
        let seq = slf.seq.read().expect("failed to read lock");
        let feature = &seq.features[slf.index];

        let length = feature.qualifiers.len();
        if item < 0 {
            item += length as isize;
        }
        if item < 0 || item >= length as isize {
            Err(PyIndexError::new_err(item))
        } else {
            let qualifier = &feature.qualifiers[item as usize];
            Py::new(
                slf.py(),
                Qualifier {
                    key: qualifier.0.clone(),
                    value: qualifier.1.clone(),
                },
            )
        }
    }
}

#[pyclass(module = "gb_io")]
#[derive(Debug)]
pub struct Qualifier {
    key: QualifierKey,
    value: Option<String>,
}

#[pymethods]
impl Qualifier {
    #[getter]
    pub fn get_key<'py>(slf: PyRef<'py, Self>) -> PyObject {
        let py = slf.py();
        PyString::new(py, slf.deref().key.deref()).into_py(py)
    }

    #[getter]
    pub fn get_value<'py>(slf: PyRef<'py, Self>) -> PyObject {
        let py = slf.py();
        match &slf.deref().value {
            None => py.None(),
            Some(s) => PyString::new(py, s.deref()).into_py(py),
        }
    }
}

// ---------------------------------------------------------------------------

/// A fast GenBank I/O library based on the ``gb-io`` Rust crate.
///
#[pymodule]
#[pyo3(name = "gb_io")]
pub fn init(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<self::Record>()?;
    m.add_class::<self::RecordReader>()?;
    m.add("__package__", "gb_io")?;
    m.add("__build__", pyo3_built!(py, built))?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("__author__", env!("CARGO_PKG_AUTHORS").replace(':', "\n"))?;

    /// Load all GenBank records from the given path or file handle.
    ///
    /// Arguments:
    ///     fh (str or file-handle): The path to a GenBank file, or a
    ///         stream that contains data serialized in GenBank format.
    ///
    /// Returns:
    ///     `list` of `Record`: A list containing all the records in the file.
    ///
    #[pyfn(m)]
    #[pyo3(name = "load", text_signature = "(fh)")]
    fn load(py: Python, fh: &PyAny) -> PyResult<Py<PyList>> {
        // extract either a path or a file-handle from the arguments
        // let path: Option<String>;
        let stream: Box<dyn Read> = if let Ok(s) = fh.cast_as::<PyString>() {
            // get a buffered reader to the resources pointed by `path`
            let bf = match std::fs::File::open(s.to_str()?) {
                Ok(f) => f,
                Err(_e) => unimplemented!("error management"),
            };
            // store the path for later
            // path = Some(s.to_str()?.to_string());
            // send the file reader to the heap.
            Box::new(bf)
        } else {
            // get a buffered reader by wrapping the given file handle
            let bf = match PyFileRead::from_ref(fh) {
                // Object is a binary file-handle: attempt to parse the
                // document and return an `OboDoc` object.
                Ok(f) => f,
                // Object is not a binary file-handle: wrap the inner error
                // into a `TypeError` and raise that error.
                Err(_e) => {
                    unimplemented!("error management")
                    // raise!(py, PyTypeError("expected path or binary file handle") from e)
                }
            };
            // extract the path from the `name` attribute
            // path = fh
            //     .getattr("name")
            //     .and_then(|n| n.downcast::<PyString>().map_err(PyErr::from))
            //     .and_then(|s| s.to_str())
            //     .map(|s| s.to_string())
            //     .ok();
            // send the Python file-handle reference to the heap.
            Box::new(bf)
        };

        // create the reader
        let reader = SeqReader::new(stream);

        // parse all records
        let records = PyList::empty(py);
        for result in reader {
            match result {
                Ok(seq) => {
                    records.append(Py::new(py, Record::from(seq))?)?;
                }
                Err(error) => {
                    // FIXME: error management
                    let msg = format!("parser failed: {}", error);
                    return Err(PyRuntimeError::new_err(msg));
                }
            }
        }

        // return records
        Ok(records.into_py(py))
    }

    /// Iterate over the GenBank records in the given file or file handle.
    ///
    /// Arguments:
    ///     fh (str or file-handle): The path to a GenBank file, or a stream
    ///         that contains data serialized in GenBank format.
    ///
    /// Returns:
    ///     `gb_io.RecordReader`: An iterator over the GenBank records in the
    ///     given file or file-handle.
    ///
    #[pyfn(m)]
    #[pyo3(name = "iter", text_signature = "(fh)")]
    fn iter(py: Python, fh: &PyAny) -> PyResult<Py<RecordReader>> {
        let reader = match fh.cast_as::<PyString>() {
            Ok(s) => RecordReader::from_path(s.to_str()?)?,
            Err(_) => RecordReader::from_handle(fh)?,
        };
        Py::new(py, reader)
    }

    Ok(())
}
