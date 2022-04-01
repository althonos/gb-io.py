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

use gb_io::reader::GbParserError;
use gb_io::reader::SeqReader;
use gb_io::seq::After;
use gb_io::seq::Before;
use gb_io::seq::Location as SeqLocation;
use gb_io::seq::Seq;
use gb_io::seq::Topology;
use gb_io::QualifierKey;
use pyo3::exceptions::PyIndexError;
use pyo3::exceptions::PyNotImplementedError;
use pyo3::exceptions::PyOSError;
use pyo3::exceptions::PyTypeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyDate;
use pyo3::types::PyDateAccess;
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

    /// `str`, optional: The molecule type of the record, or `None`.
    #[getter]
    fn get_molecule_type(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        let seq = slf.seq.read().expect("cannot read lock");
        match &seq.molecule_type {
            None => Ok(slf.py().None()),
            Some(v) => Ok(PyString::new(slf.py(), v).into_py(slf.py())),
        }
    }

    #[setter]
    fn set_molecule_type(slf: PyRefMut<'_, Self>, molecule_type: Option<String>) -> PyResult<()> {
        let mut seq = slf.seq.write().expect("cannot write lock");
        seq.molecule_type = molecule_type;
        Ok(())
    }

    /// `str`: The division this record is stored under in GenBank.
    #[getter]
    fn get_division(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        let seq = slf.seq.read().expect("cannot read lock");
        Ok(PyString::new(slf.py(), &seq.division).into_py(slf.py()))
    }

    #[setter]
    fn set_division(slf: PyRefMut<'_, Self>, division: String) -> PyResult<()> {
        let mut seq = slf.seq.write().expect("cannot write lock");
        seq.division = division;
        Ok(())
    }

    /// `str`, optional: Keywords related to the record, or `None`.
    #[getter]
    fn get_keywords(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        let seq = slf.seq.read().expect("cannot read lock");
        match &seq.keywords {
            None => Ok(slf.py().None()),
            Some(v) => Ok(PyString::new(slf.py(), v).into_py(slf.py())),
        }
    }

    #[setter]
    fn set_keywords(slf: PyRefMut<'_, Self>, keywords: Option<String>) -> PyResult<()> {
        let mut seq = slf.seq.write().expect("cannot write lock");
        seq.keywords = keywords;
        Ok(())
    }

    /// `~datetime.date`, optional: The date this record was submitted, or `None`.
    #[getter]
    fn get_date(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let seq = slf.seq.read().expect("cannot read lock");
        match &seq.date {
            None => Ok(py.None()),
            Some(dt) => {
                let date = PyDate::new(py, dt.year(), dt.month() as u8, dt.day() as u8)?;
                Ok(date.into_py(py))
            }
        }
    }

    #[setter]
    fn set_date(slf: PyRefMut<'_, Self>, date: Option<&PyDate>) -> PyResult<()> {
        let mut seq = slf.seq.write().expect("cannot write lock");
        if let Some(dt) = date {
            let year = dt.get_year();
            let month = dt.get_month() as u32;
            let day = dt.get_day() as u32;
            if let Ok(date) = gb_io::seq::Date::from_ymd(year, month, day) {
                seq.date = Some(date);
            } else {
                return Err(PyValueError::new_err("invalid date"));
            }
        } else {
            seq.date = None;
        }
        Ok(())
    }

    /// `bytes`: The sequence of the record in lowercase, as raw ASCII.
    #[getter]
    fn get_sequence(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        let seq = slf.seq.read().expect("failed to read lock");
        Ok(PyBytes::new(slf.py(), &seq.seq).into())
    }

    /// `~gb_io.Features`: A collection of features within the record.
    #[getter]
    fn get_features(slf: PyRef<'_, Self>) -> PyResult<Py<Features>> {
        Py::new(
            slf.py(),
            Features {
                seq: slf.seq.clone(),
            },
        )
    }

    // TODO: len, source, dblink, references, comments, contig,
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
            Err(PyIndexError::new_err(slf.index))
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

    #[getter]
    fn get_location<'py>(slf: PyRef<'py, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let seq = slf.seq.read().expect("failed to read lock");
        if slf.index < seq.features.len() {
            Location::convert(py, &seq.features[slf.index].location)
        } else {
            Err(PyIndexError::new_err(slf.index))
        }
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

#[pyclass(module = "gb_io", subclass)]
#[derive(Debug)]
pub struct Location;

impl Location {
    fn convert(py: Python<'_>, location: &SeqLocation) -> PyResult<PyObject> {
        match location {
            SeqLocation::Range((start, Before(before)), (end, After(after))) => {
                Py::new(py, Range::__new__(*start, *end, *before, *after)).map(|x| x.to_object(py))
            }
            SeqLocation::Between(start, end) => {
                Py::new(py, Between::__new__(*start, *end)).map(|x| x.to_object(py))
            }
            SeqLocation::Complement(inner_location) => Location::convert(py, inner_location)
                .and_then(|inner| Py::new(py, Complement::__new__(inner)))
                .map(|x| x.to_object(py)),
            _ => Err(PyNotImplementedError::new_err(format!(
                "conversion of {:?}",
                location
            ))),
        }
    }
}

#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct Range {
    #[pyo3(get)]
    start: i64,
    #[pyo3(get)]
    end: i64,
    #[pyo3(get)]
    before: bool,
    #[pyo3(get)]
    after: bool,
}

impl From<&Range> for SeqLocation {
    fn from(range: &Range) -> SeqLocation {
        SeqLocation::Range(
            (range.start, Before(range.before)),
            (range.end, After(range.after)),
        )
    }
}

#[pymethods]
impl Range {
    #[new]
    #[args(before = "false", after = "false")]
    fn __new__(start: i64, end: i64, before: bool, after: bool) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Location).add_subclass(Self {
            start: start,
            end: end,
            before: before,
            after: after,
        })
    }

    fn __repr__(&self) -> String {
        match (self.before, self.after) {
            (false, false) => format!("Range({}, {})", self.start, self.end),
            (true, false) => format!("Range({}, {}, before=True)", self.start, self.end),
            (false, true) => format!("Range({}, {}, after=True)", self.start, self.end),
            (true, true) => format!(
                "Range({}, {}, before=True, after=True)",
                self.start, self.end
            ),
        }
    }
}

#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct Between {
    start: i64,
    end: i64,
}

#[pymethods]
impl Between {
    #[new]
    fn __new__(start: i64, end: i64) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Location).add_subclass(Self {
            start: start,
            end: end,
        })
    }

    fn __repr__(&self) -> String {
        format!("Between({}, {})", self.start, self.end)
    }
}

#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct Complement {
    location: PyObject,
}

#[pymethods]
impl Complement {
    #[new]
    fn __new__(location: PyObject) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Location).add_subclass(Self { location })
    }

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let s = PyString::new(py, "Complement({})")
            .call_method1("format", (Py::clone_ref(&slf.location, py),))?;
        Ok(s.to_object(py))
    }
}

// ---------------------------------------------------------------------------

/// A fast GenBank I/O library based on the ``gb-io`` Rust crate.
///
#[pymodule]
#[pyo3(name = "gb_io")]
pub fn init(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<self::Location>()?;
    m.add_class::<self::Range>()?;
    m.add_class::<self::Complement>()?;
    m.add_class::<self::Between>()?;
    m.add_class::<self::Qualifier>()?;
    m.add_class::<self::Qualifiers>()?;
    m.add_class::<self::Feature>()?;
    m.add_class::<self::Features>()?;
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
                Err(e) => {
                    return match e.raw_os_error() {
                        Some(code) => Err(PyOSError::new_err((code, e.to_string()))),
                        None => Err(PyOSError::new_err(e.to_string())),
                    }
                }
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
                Err(e) => {
                    let err = PyTypeError::new_err("expected path or binary file handle");
                    err.set_cause(py, Some(e));
                    return Err(err);
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
                Err(GbParserError::Io(e)) => {
                    return match e.raw_os_error() {
                        Some(code) => Err(PyOSError::new_err((code, e.to_string()))),
                        None => match PyErr::take(py) {
                            Some(e) => Err(e),
                            None => Err(PyOSError::new_err(e.to_string())),
                        },
                    };
                }
                Err(GbParserError::SyntaxError(e)) => {
                    let msg = format!("parser failed: {}", e);
                    return Err(PyValueError::new_err(msg));
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
