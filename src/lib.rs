extern crate gb_io;
extern crate libc;
extern crate pyo3;
extern crate pyo3_built;

mod built;
mod iter;
mod pyfile;

use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
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
use gb_io::writer::SeqWriter;
use gb_io::QualifierKey;
use pyo3::exceptions::PyIOError;
use pyo3::exceptions::PyIndexError;
use pyo3::exceptions::PyNotImplementedError;
use pyo3::exceptions::PyOSError;
use pyo3::exceptions::PyTypeError;
use pyo3::exceptions::PyValueError;
use pyo3::intern;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyDate;
use pyo3::types::PyDateAccess;
use pyo3::types::PyDict;
use pyo3::types::PyIterator;
use pyo3::types::PyList;
use pyo3::types::PyString;
use pyo3::types::PyTuple;
use pyo3_built::pyo3_built;

use self::iter::RecordReader;
use self::pyfile::PyFileRead;
use self::pyfile::PyFileWrite;

// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct PyInterner {
    cache: RwLock<HashMap<String, Py<PyString>>>,
}

impl PyInterner {
    pub fn intern<S: AsRef<str>>(&self, py: Python, s: S) -> Py<PyString> {
        let key = s.as_ref();
        if let Some(pystring) = self
            .cache
            .read()
            .expect("failed to acquired cache")
            .get(key)
        {
            return pystring.clone();
        }
        let mut cache = self.cache.write().expect("failed to acquire cache");
        let pystring = Py::from(PyString::new(py, key));
        cache.insert(key.into(), pystring.clone());
        pystring
    }
}

pub trait Convert: Sized {
    type Output;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Self::Output>;
    fn convert(self, py: Python) -> PyResult<Self::Output> {
        self.convert_with(py, &mut PyInterner::default())
    }
}

impl<T: Convert> Convert for Vec<T>
where
    T: Convert,
    <T as Convert>::Output: ToPyObject,
{
    type Output = Py<PyList>;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Self::Output> {
        let l = PyList::empty(py);
        for elem in self.into_iter() {
            l.append(elem.convert_with(py, interner)?)?;
        }
        Ok(Py::from(l))
    }
}

// ---------------------------------------------------------------------------

/// A single GenBank record.
#[pyclass(module = "gb_io")]
#[derive(Debug, Clone)]
pub struct Record {
    // seq: Arc<RwLock<Seq>>,
    #[pyo3(get, set)]
    name: Option<String>,
    topology: Topology,
    // date: Option<Date>,
    #[pyo3(get, set)]
    len: Option<usize>,
    molecule_type: Option<String>,
    #[pyo3(get)]
    division: String,
    #[pyo3(get, set)]
    definition: Option<String>,
    #[pyo3(get, set)]
    accession: Option<String>,
    #[pyo3(get, set)]
    version: Option<String>,
    // source: Option<Source>,
    dblink: Option<String>,
    keywords: Option<String>,
    // references: Vec<Arc<Reference>>,
    comments: Vec<String>,
    sequence: Vec<u8>,
    // contig: Option<Location>,
    #[pyo3(get, set)]
    features: Py<PyList>,
}

#[pymethods]
impl Record {
    // /// Create a new record.
    // #[new]
    // #[pyo3(signature = (sequence, *, name = None, division = String::from("UNK"), circular = false, accession = None, version = None))]
    // fn __init__<'py>(
    //     sequence: &'py PyAny,
    //     name: Option<String>,
    //     division: String,
    //     circular: bool,
    //     accession: Option<String>,
    //     version: Option<String>,
    // ) -> PyResult<PyClassInitializer<Self>> {
    //     let seq = if let Ok(sequence_str) = sequence.downcast::<PyString>() {
    //         sequence_str.to_str()?.as_bytes().to_vec()
    //     } else if let Ok(sequence_bytes) = sequence.downcast::<PyBytes>() {
    //         sequence_bytes.as_bytes().to_vec()
    //     } else {
    //         return Err(PyTypeError::new_err("Expected str or bytes for `sequence`"));
    //     };

    //     let topology = match circular {
    //         true => Topology::Circular,
    //         false => Topology::Linear,
    //     };

    //     let record = Record::from(Seq {
    //         name,
    //         division,
    //         seq,
    //         topology,
    //         contig: None,
    //         features: Vec::new(),
    //         comments: Vec::new(),
    //         date: None,
    //         len: None,
    //         molecule_type: None,
    //         definition: None,
    //         accession,
    //         version,
    //         source: None,
    //         dblink: None,
    //         keywords: None,
    //         references: Vec::new(),
    //     });
    //     Ok(record.into())
    // }

    // /// `str`, optional: The name of the record, or `None`.
    // #[getter]
    // fn get_name(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
    //     let seq = slf.seq.read().expect("cannot read lock");
    //     match &seq.name {
    //         None => Ok(slf.py().None()),
    //         Some(n) => Ok(PyString::new(slf.py(), &n).into_py(slf.py())),
    //     }
    // }

    // #[setter]
    // fn set_name(slf: PyRefMut<'_, Self>, name: Option<String>) -> PyResult<()> {
    //     let mut seq = slf.seq.write().expect("cannot write lock");
    //     seq.name = name;
    //     Ok(())
    // }

    // /// `str`: The topology of the record.
    // #[getter]
    // fn get_topology(slf: PyRef<'_, Self>) -> PyResult<&str> {
    //     let seq = slf.seq.read().expect("cannot read lock");
    //     match &seq.topology {
    //         Topology::Linear => Ok("linear"),
    //         Topology::Circular => Ok("circular"),
    //     }
    // }

    // #[setter]
    // fn set_topology(slf: PyRefMut<'_, Self>, topology: &str) -> PyResult<()> {
    //     let mut seq = slf.seq.write().expect("cannot write lock");
    //     match topology {
    //         "linear" => {
    //             seq.topology = Topology::Linear;
    //             Ok(())
    //         }
    //         "circular" => {
    //             seq.topology = Topology::Circular;
    //             Ok(())
    //         }
    //         other => {
    //             let message = format!("invalid topology: {:?}", other);
    //             Err(PyValueError::new_err(message))
    //         }
    //     }
    // }

    // /// `str`, optional: The definition of the record, or `None`.
    // #[getter]
    // fn get_definition(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
    //     let seq = slf.seq.read().expect("cannot read lock");
    //     match &seq.definition {
    //         None => Ok(slf.py().None()),
    //         Some(n) => Ok(PyString::new(slf.py(), &n).into_py(slf.py())),
    //     }
    // }

    // #[setter]
    // fn set_definition(slf: PyRefMut<'_, Self>, definition: Option<String>) -> PyResult<()> {
    //     let mut seq = slf.seq.write().expect("cannot write lock");
    //     seq.definition = definition;
    //     Ok(())
    // }

    // /// `str`, optional: The accession of the record, or `None`.
    // #[getter]
    // fn get_accession(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
    //     let seq = slf.seq.read().expect("cannot read lock");
    //     match &seq.accession {
    //         None => Ok(slf.py().None()),
    //         Some(n) => Ok(PyString::new(slf.py(), &n).into_py(slf.py())),
    //     }
    // }

    // #[setter]
    // fn set_accession(slf: PyRefMut<'_, Self>, accession: Option<String>) -> PyResult<()> {
    //     let mut seq = slf.seq.write().expect("cannot write lock");
    //     seq.accession = accession;
    //     Ok(())
    // }

    // /// `str`, optional: The version of the record, or `None`.
    // #[getter]
    // fn get_version(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
    //     let seq = slf.seq.read().expect("cannot read lock");
    //     match &seq.version {
    //         None => Ok(slf.py().None()),
    //         Some(v) => Ok(PyString::new(slf.py(), v).into_py(slf.py())),
    //     }
    // }

    // #[setter]
    // fn set_version(slf: PyRefMut<'_, Self>, version: Option<String>) -> PyResult<()> {
    //     let mut seq = slf.seq.write().expect("cannot write lock");
    //     seq.version = version;
    //     Ok(())
    // }

    // /// `str`, optional: The molecule type of the record, or `None`.
    // #[getter]
    // fn get_molecule_type(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
    //     let seq = slf.seq.read().expect("cannot read lock");
    //     match &seq.molecule_type {
    //         None => Ok(slf.py().None()),
    //         Some(v) => Ok(PyString::new(slf.py(), v).into_py(slf.py())),
    //     }
    // }

    // #[setter]
    // fn set_molecule_type(slf: PyRefMut<'_, Self>, molecule_type: Option<String>) -> PyResult<()> {
    //     let mut seq = slf.seq.write().expect("cannot write lock");
    //     seq.molecule_type = molecule_type;
    //     Ok(())
    // }

    // /// `str`: The division this record is stored under in GenBank.
    // #[getter]
    // fn get_division(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
    //     let seq = slf.seq.read().expect("cannot read lock");
    //     Ok(PyString::new(slf.py(), &seq.division).into_py(slf.py()))
    // }

    // #[setter]
    // fn set_division(slf: PyRefMut<'_, Self>, division: String) -> PyResult<()> {
    //     let mut seq = slf.seq.write().expect("cannot write lock");
    //     seq.division = division;
    //     Ok(())
    // }

    // /// `str`, optional: Keywords related to the record, or `None`.
    // #[getter]
    // fn get_keywords(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
    //     let seq = slf.seq.read().expect("cannot read lock");
    //     match &seq.keywords {
    //         None => Ok(slf.py().None()),
    //         Some(v) => Ok(PyString::new(slf.py(), v).into_py(slf.py())),
    //     }
    // }

    // #[setter]
    // fn set_keywords(slf: PyRefMut<'_, Self>, keywords: Option<String>) -> PyResult<()> {
    //     let mut seq = slf.seq.write().expect("cannot write lock");
    //     seq.keywords = keywords;
    //     Ok(())
    // }

    // /// `~datetime.date`, optional: The date this record was submitted, or `None`.
    // #[getter]
    // fn get_date(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
    //     let py = slf.py();
    //     let seq = slf.seq.read().expect("cannot read lock");
    //     match &seq.date {
    //         None => Ok(py.None()),
    //         Some(dt) => {
    //             let date = PyDate::new(py, dt.year(), dt.month() as u8, dt.day() as u8)?;
    //             Ok(date.into_py(py))
    //         }
    //     }
    // }

    // #[setter]
    // fn set_date(slf: PyRefMut<'_, Self>, date: Option<&PyDate>) -> PyResult<()> {
    //     let mut seq = slf.seq.write().expect("cannot write lock");
    //     if let Some(dt) = date {
    //         let year = dt.get_year();
    //         let month = dt.get_month() as u32;
    //         let day = dt.get_day() as u32;
    //         if let Ok(date) = gb_io::seq::Date::from_ymd(year, month, day) {
    //             seq.date = Some(date);
    //         } else {
    //             return Err(PyValueError::new_err("invalid date"));
    //         }
    //     } else {
    //         seq.date = None;
    //     }
    //     Ok(())
    // }

    /// `bytes`: The sequence of the record in lowercase, as raw ASCII.
    #[getter]
    fn get_sequence(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        // let seq = slf.seq.read().expect("failed to read lock");
        Ok(PyBytes::new(slf.py(), &slf.sequence).into())
    }

    // /// `~gb_io.Features`: A collection of features within the record.
    // #[getter]
    // fn get_features(slf: PyRef<'_, Self>) -> PyResult<Py<Features>> {
    //     Py::new(
    //         slf.py(),
    //         Features {
    //             seq: slf.seq.clone(),
    //         },
    //     )
    // }

    // TODO: len, source, dblink, references, comments, contig,
}

impl Convert for gb_io::seq::Seq {
    type Output = Py<Record>;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Self::Output> {
        Py::new(
            py,
            Record {
                name: self.name,
                topology: self.topology,
                len: self.len,
                molecule_type: self.molecule_type,
                division: self.division,
                definition: self.definition,
                accession: self.accession,
                version: self.version,
                dblink: self.dblink,
                keywords: self.keywords,
                comments: self.comments,
                sequence: self.seq,
                features: self.features.convert_with(py, interner)?,
            },
        )
    }
}

impl From<Record> for gb_io::seq::Seq {
    fn from(record: Record) -> Self {
        unimplemented!()
    }
}

// ---------------------------------------------------------------------------

/// The source of a GenBank record.
#[pyclass(module = "gb_io")]
#[derive(Debug)]
pub struct Source {
    seq: Arc<RwLock<Seq>>,
}

#[pymethods]
impl Source {
    #[getter]
    fn get_source<'py>(slf: PyRef<'py, Self>) -> PyObject {
        let py = slf.py();
        let seq = slf.seq.read().expect("failed to read lock");
        PyString::new(py, &seq.source.as_ref().unwrap().source).to_object(py)
    }

    #[getter]
    fn get_organism<'py>(slf: PyRef<'py, Self>) -> Option<PyObject> {
        let py = slf.py();
        let seq = slf.seq.read().expect("failed to read lock");
        if let Some(organism) = &seq.source.as_ref().unwrap().organism {
            Some(PyString::new(py, organism).to_object(py))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------

#[pyclass(module = "gb_io")]
#[derive(Debug, Clone)]
pub struct Feature {
    #[pyo3(get, set)]
    kind: Py<PyString>,
    #[pyo3(get, set)]
    qualifiers: Py<PyList>,
    #[pyo3(get, set)]
    location: PyObject,
}

#[pymethods]
impl Feature {}

impl Convert for gb_io::seq::Feature {
    type Output = Py<Feature>;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Self::Output> {
        Py::new(
            py,
            Feature {
                kind: interner.intern(py, self.kind),
                location: self.location.convert_with(py, interner)?,
                qualifiers: self.qualifiers.convert_with(py, interner)?,
            },
        )
    }
}

// impl From<gb_io::seq::Feature> for Feature {
//     fn from(feature: gb_io::seq::Feature) -> Self {

//     }
// }

// #[pyclass(module = "gb_io")]
// #[derive(Debug)]
// pub struct Qualifiers {
//     key: gb_io::QualifierKey,
//     value: Option<String>,
// }

// #[pymethods]
// impl Qualifiers {
//     /// Group the qualifiers by key into a dictionary.
//     fn to_dict(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
//         let seq = slf.seq.read().expect("failed to read lock");
//         let feature = &seq.features[slf.index];

//         let dict = PyDict::new(slf.py());
//         for (key, value) in feature.qualifiers.iter() {
//             if let Some(v) = value {
//                 let l = dict
//                     .call_method1("setdefault", (key.deref(), PyList::empty(slf.py())))?
//                     .downcast::<PyList>()?;
//                 l.append(PyString::new(slf.py(), v))?;
//             }
//         }

//         Ok(dict.into_py(slf.py()))
//     }

//     fn __len__(slf: PyRef<'_, Self>) -> PyResult<usize> {
//         let seq = slf.seq.read().expect("failed to read lock");
//         let feature = &seq.features[slf.index];
//         Ok(feature.qualifiers.len())
//     }

//     fn __getitem__(slf: PyRef<'_, Self>, mut item: isize) -> PyResult<Py<Qualifier>> {
//         let seq = slf.seq.read().expect("failed to read lock");
//         let feature = &seq.features[slf.index];

//         let length = feature.qualifiers.len();
//         if item < 0 {
//             item += length as isize;
//         }
//         if item < 0 || item >= length as isize {
//             Err(PyIndexError::new_err(item))
//         } else {
//             let qualifier = &feature.qualifiers[item as usize];
//             Py::new(
//                 slf.py(),
//                 Qualifier {
//                     key: qualifier.0.clone(),
//                     value: qualifier.1.clone(),
//                 },
//             )
//         }
//     }
// }

#[pyclass(module = "gb_io")]
#[derive(Debug)]
pub struct Qualifier {
    #[pyo3(get, set)]
    key: Py<PyString>,
    #[pyo3(get, set)]
    value: Option<String>,
}

impl Qualifier {
    pub fn new<S>(key: Py<PyString>, value: S) -> Self
    where
        S: Into<Option<String>>,
    {
        Self {
            key,
            value: value.into(),
        }
    }
}

impl Convert for (QualifierKey, Option<String>) {
    type Output = Py<Qualifier>;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Self::Output> {
        Py::new(py, Qualifier::new(interner.intern(py, self.0), self.1))
    }
}

// ---------------------------------------------------------------------------

#[pyclass(module = "gb_io", subclass)]
#[derive(Debug)]
pub struct Location;

// impl Location {
//     fn convert(py: Python<'_>, location: &SeqLocation) -> PyResult<PyObject> {
//         macro_rules! convert_vec {
//             ($ty:ident, $inner:expr) => {{
//                 let objects: PyObject = $inner
//                     .iter()
//                     .map(|loc| Location::convert(py, loc))
//                     .collect::<PyResult<Vec<PyObject>>>()
//                     .map(|objects| PyList::new(py, objects))
//                     .map(|list| list.to_object(py))?;
//                 Py::new(py, Join::__new__(objects)).map(|x| x.to_object(py))
//             }};
//         }

//         match location {
//             SeqLocation::Range((start, Before(before)), (end, After(after))) => {
//                 Py::new(py, Range::__new__(*start, *end, *before, *after)).map(|x| x.to_object(py))
//             }
//             SeqLocation::Between(start, end) => {
//                 Py::new(py, Between::__new__(*start, *end)).map(|x| x.to_object(py))
//             }
//             SeqLocation::Complement(inner_location) => Location::convert(py, inner_location)
//                 .and_then(|inner| Py::new(py, Complement::__new__(inner)))
//                 .map(|x| x.to_object(py)),
//             SeqLocation::Join(inner_locations) => convert_vec!(Join, inner_locations),
//             SeqLocation::Order(inner_locations) => convert_vec!(Order, inner_locations),
//             SeqLocation::Bond(inner_locations) => convert_vec!(Bond, inner_locations),
//             SeqLocation::OneOf(inner_locations) => convert_vec!(OneOf, inner_locations),
//             SeqLocation::External(accession, location) => {
//                 let loc = location.clone().map(|x| Location::convert(py, &x)).transpose()?;
//                 Py::new(py, External::__new__(accession.clone(), loc)).map(|x| x.to_object(py))
//             }
//             _ => Err(PyNotImplementedError::new_err(format!(
//                 "conversion of {:?}",
//                 location
//             ))),
//         }
//     }
// }

impl Convert for gb_io::seq::Location {
    type Output = PyObject;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Self::Output> {
        macro_rules! convert_vec {
            ($ty:ident, $inner:expr) => {{
                let objects: PyObject = $inner
                    .into_iter()
                    .map(|loc| loc.convert_with(py, interner))
                    .collect::<PyResult<Vec<PyObject>>>()
                    .map(|objects| PyList::new(py, objects))
                    .map(|list| list.to_object(py))?;
                Py::new(py, Join::__new__(objects)).map(|x| x.to_object(py))
            }};
        }

        match self {
            SeqLocation::Range((start, Before(before)), (end, After(after))) => {
                Py::new(py, Range::__new__(start, end, before, after)).map(|x| x.to_object(py))
            }
            SeqLocation::Between(start, end) => {
                Py::new(py, Between::__new__(start, end)).map(|x| x.to_object(py))
            }
            SeqLocation::Complement(inner_location) => (*inner_location)
                .convert_with(py, interner)
                .and_then(|inner| Py::new(py, Complement::__new__(inner)))
                .map(|x| x.to_object(py)),
            SeqLocation::Join(inner_locations) => convert_vec!(Join, inner_locations),
            SeqLocation::Order(inner_locations) => convert_vec!(Order, inner_locations),
            SeqLocation::Bond(inner_locations) => convert_vec!(Bond, inner_locations),
            SeqLocation::OneOf(inner_locations) => convert_vec!(OneOf, inner_locations),
            SeqLocation::External(accession, location) => {
                let loc = location.map(|x| x.convert_with(py, interner)).transpose()?;
                Py::new(py, External::__new__(accession, loc)).map(|x| x.to_object(py))
            }
            _ => Err(PyNotImplementedError::new_err(format!(
                "conversion of {:?}",
                self
            ))),
        }
    }
}

#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct Range {
    #[pyo3(get, set)]
    /// `int`: The start of the range of positions.
    start: i64,
    #[pyo3(get, set)]
    /// `int`: The end of the range of positions.
    end: i64,
    #[pyo3(get, set)]
    /// `bool`: Whether the range start before the given ``start`` index.
    before: bool,
    #[pyo3(get, set)]
    /// `bool`: Whether the range extends after the given ``end`` index.
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
    #[pyo3(signature = (start, end, before = false, after = false))]
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
    #[pyo3(get)]
    start: i64,
    #[pyo3(get)]
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

    #[getter]
    fn get_start<'py>(slf: PyRef<'py, Self>) -> PyResult<i32> {
        let py = slf.py();
        slf.location
            .getattr(py, "end")
            .and_then(|end| end.extract(py))
    }

    #[getter]
    fn get_end<'py>(slf: PyRef<'py, Self>) -> PyResult<i32> {
        let py = slf.py();
        slf.location
            .getattr(py, "start")
            .and_then(|start| start.extract(py))
    }
}

#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct Join {
    locations: PyObject,
}

#[pymethods]
impl Join {
    #[new]
    fn __new__(locations: PyObject) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Location).add_subclass(Self { locations })
    }

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let s = PyString::new(py, "Join({})").call_method1("format", (&slf.locations,))?;
        Ok(s.to_object(py))
    }

    #[getter]
    fn get_start<'py>(slf: PyRef<'py, Self>) -> PyResult<i32> {
        let py = slf.py();
        let mut min: Option<i32> = None;
        for obj in slf.locations.downcast::<PyList>(py)? {
            let start = obj.getattr("start")?.extract::<i32>()?;
            min = match min {
                Some(i) if i < start => Some(i),
                _ => Some(start),
            }
        }
        min.ok_or(PyValueError::new_err(
            "cannot get start coordinate of empty list of locations",
        ))
    }

    #[getter]
    fn get_end<'py>(slf: PyRef<'py, Self>) -> PyResult<i32> {
        let py = slf.py();
        let mut min: Option<i32> = None;
        for obj in slf.locations.downcast::<PyList>(py)? {
            let end = obj.getattr("end")?.extract::<i32>()?;
            min = match min {
                Some(i) if i > end => Some(i),
                _ => Some(end),
            }
        }
        min.ok_or(PyValueError::new_err(
            "cannot get end coordinate of empty list of locations",
        ))
    }
}

#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct Order {
    locations: PyObject,
}

#[pymethods]
impl Order {
    #[new]
    fn __new__(locations: PyObject) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Location).add_subclass(Self { locations })
    }

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let s = PyString::new(py, "Order({})").call_method1("format", (&slf.locations,))?;
        Ok(s.to_object(py))
    }
}

#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct Bond {
    locations: PyObject,
}

#[pymethods]
impl Bond {
    #[new]
    fn __new__(locations: PyObject) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Location).add_subclass(Self { locations })
    }

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let s = PyString::new(py, "Bond({})").call_method1("format", (&slf.locations,))?;
        Ok(s.to_object(py))
    }
}

#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct OneOf {
    locations: PyObject,
}

#[pymethods]
impl OneOf {
    #[new]
    fn __new__(locations: PyObject) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Location).add_subclass(Self { locations })
    }

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let s = PyString::new(py, "OneOf({})").call_method1("format", (&slf.locations,))?;
        Ok(s.to_object(py))
    }
}

#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct External {
    accession: String,
    location: Option<PyObject>,
}

#[pymethods]
impl External {
    #[new]
    fn __new__(accession: String, location: Option<PyObject>) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Location).add_subclass(Self {
            accession,
            location,
        })
    }

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let s = match &slf.location {
            Some(s) => {
                PyString::new(py, "External({}, {})").call_method1("format", (&slf.accession, s))?
            }
            None => PyString::new(py, "External({})").call_method1("format", (&slf.accession,))?,
        };
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
    m.add_class::<self::Join>()?;
    m.add_class::<self::Order>()?;
    m.add_class::<self::Bond>()?;
    m.add_class::<self::OneOf>()?;
    m.add_class::<self::External>()?;
    // m.add_class::<self::Qualifier>()?;
    // m.add_class::<self::Qualifiers>()?;
    m.add_class::<self::Feature>()?;
    // m.add_class::<self::Features>()?;
    m.add_class::<self::Record>()?;
    m.add_class::<self::RecordReader>()?;
    m.add("__package__", "gb_io")?;
    m.add("__build__", pyo3_built!(py, built))?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("__author__", env!("CARGO_PKG_AUTHORS").replace(':', "\n"))?;

    /// Load all GenBank records from the given path or file handle.
    ///
    /// Arguments:
    ///     fh (`str` or file-handle): The path to a GenBank file, or a
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
        let stream: Box<dyn Read> = if let Ok(s) = fh.downcast::<PyString>() {
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
            // send the Python file-handle reference to the heap.
            Box::new(bf)
        };

        // create the reader
        let reader = SeqReader::new(stream);

        // parse all records
        let mut interner = PyInterner::default();
        let records = PyList::empty(py);
        for result in reader {
            match result {
                Ok(seq) => {
                    records.append(Py::new(py, seq.convert_with(py, &mut interner)?)?)?;
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
    ///     fh (`str` or file-handle): The path to a GenBank file, or a
    ///         stream that contains data serialized in GenBank format.
    ///
    /// Returns:
    ///     `~gb_io.RecordReader`: An iterator over the GenBank records in
    ///     the given file or file-handle.
    ///
    #[pyfn(m)]
    #[pyo3(name = "iter", text_signature = "(fh)")]
    fn iter(py: Python, fh: &PyAny) -> PyResult<Py<RecordReader>> {
        let reader = match fh.downcast::<PyString>() {
            Ok(s) => RecordReader::from_path(s.to_str()?)?,
            Err(_) => RecordReader::from_handle(fh)?,
        };
        Py::new(py, reader)
    }

    /// Write one or more GenBank records to the given path or file handle.
    ///
    /// Arguments:
    ///     records (`Record` or iterable of `Record`): The records to write
    ///         to the file.
    ///     fh (str or file-handle): The path to a GenBank file, or a stream
    ///         that contains data serialized in GenBank format.
    ///
    /// Keywords Arguments:
    ///     escape_locus (`bool`): Pass `True` to escape any whitespace in
    ///         the locus name with an underscore character.
    ///     truncate_locus (`bool`): Pass `True` to trim the locus fields
    ///          so that the locus line is no longer than 79 characters.
    ///
    /// .. versionadded:: 0.2.0
    #[pyfunction]
    #[pyo3(
        name = "dump",
        signature = (records, fh, escape_locus = false, truncate_locus = false),
        text_signature = "(records, fh, *, escape_locus=False, truncate_locus=False)"
    )]
    fn dump(
        py: Python,
        records: &PyAny,
        fh: &PyAny,
        escape_locus: bool,
        truncate_locus: bool,
    ) -> PyResult<()> {
        // extract either a path or a file-handle from the arguments
        let stream: Box<dyn Write> = if let Ok(s) = fh.downcast::<PyString>() {
            // get a buffered reader to the resources pointed by `path`
            let bf = match std::fs::File::create(s.to_str()?) {
                Ok(f) => f,
                Err(e) => {
                    return match e.raw_os_error() {
                        Some(code) => Err(PyOSError::new_err((code, e.to_string()))),
                        None => Err(PyOSError::new_err(e.to_string())),
                    }
                }
            };
            // send the file reader to the heap.
            Box::new(bf)
        } else {
            // get a buffered writer by wrapping the file handle
            let bf = match PyFileWrite::from_ref(fh) {
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
            // send the Python file-handle reference to the heap.
            Box::new(bf)
        };

        // create the writer
        let mut writer = SeqWriter::new(stream);
        writer.truncate_locus(truncate_locus);
        writer.escape_locus(escape_locus);

        // if a single record was given, wrap it in an iterable
        let it = if let Ok(record) = records.extract::<Py<Record>>() {
            PyIterator::from_object(PyTuple::new(py, [record]))?
        } else {
            PyIterator::from_object(records)?
        };

        // write sequences
        for result in it {
            // make sure we received a Record object
            let record = result?.extract::<Py<Record>>()?;
            let cell = record.as_ref(py);
            let cellref = cell.borrow();
            // get the seq object
            let seq = (*cellref).clone().into();
            // write the seq
            writer.write(&seq).map_err(|err| match err.raw_os_error() {
                Some(code) => PyIOError::new_err((code, err.to_string())),
                None => PyIOError::new_err(err.to_string()),
            })?;
        }

        Ok(())
    }

    Ok(())
}
