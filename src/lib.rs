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
use std::ops::DerefMut;
use std::sync::RwLock;

use gb_io::reader::GbParserError;
use gb_io::reader::SeqReader;
use gb_io::seq::After;
use gb_io::seq::Before;
use gb_io::seq::Location as SeqLocation;
use gb_io::seq::Topology;
use gb_io::writer::SeqWriter;
use pyo3::exceptions::PyIOError;
use pyo3::exceptions::PyNotImplementedError;
use pyo3::exceptions::PyOSError;
use pyo3::exceptions::PyTypeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::pyclass::PyClass;
use pyo3::types::PyBytes;
use pyo3::types::PyDate;
use pyo3::types::PyDateAccess;
use pyo3::types::PyIterator;
use pyo3::types::PyList;
use pyo3::types::PyString;
use pyo3::types::PyTuple;
use pyo3::PyNativeType;
use pyo3::PyTypeInfo;
use pyo3_built::pyo3_built;

use self::iter::RecordReader;
use self::pyfile::PyFileRead;
use self::pyfile::PyFileWrite;

// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct PyInterner {
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

/// A trait for types that can be converted to an equivalent Python type.
trait Convert: Sized {
    type Output;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Py<Self::Output>>;
    fn convert(self, py: Python) -> PyResult<Py<Self::Output>> {
        self.convert_with(py, &mut PyInterner::default())
    }
}

impl<T: Convert> Convert for Vec<T> {
    type Output = PyList;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        let converted = self
            .into_iter()
            .map(|elem| elem.convert_with(py, interner))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Py::from(PyList::new(py, converted)))
    }
}

/// A trait for types that can be extracted from an equivalent Python type.
trait Extract: Convert {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self>;
}

impl<T: Extract> Extract for Vec<T>
where
    Py<<T as Convert>::Output>: for<'py> FromPyObject<'py>,
{
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let list = object.as_ref(py);
        list.into_iter()
            .map(|elem| T::extract(py, elem.extract()?))
            .collect()
    }
}
// ---------------------------------------------------------------------------

/// A trait for obtaining a temporary value from a type.
trait Temporary: Sized {
    fn temporary() -> Self;
}

impl Temporary for gb_io::seq::Date {
    fn temporary() -> Self {
        gb_io::seq::Date::from_ymd(1970, 1, 1).unwrap()
    }
}

impl Temporary for gb_io::QualifierKey {
    fn temporary() -> Self {
        gb_io::QualifierKey::from("gene")
    }
}

impl Temporary for gb_io::FeatureKind {
    fn temporary() -> Self {
        gb_io::FeatureKind::from("locus_tag")
    }
}

impl Temporary for gb_io::seq::Location {
    fn temporary() -> Self {
        gb_io::seq::Location::Between(0, 1)
    }
}

impl<T> Temporary for Vec<T> {
    fn temporary() -> Self {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Coa<T: Convert> {
    Owned(T),
    Shared(Py<<T as Convert>::Output>),
}

impl<T: Convert + Temporary> Coa<T> {
    fn to_shared(&mut self, py: Python) -> PyResult<Py<<T as Convert>::Output>> {
        match self {
            Coa::Shared(pyref) => return Ok(pyref.clone_ref(py)),
            Coa::Owned(value) => {
                let pyref = std::mem::replace(value, Temporary::temporary()).convert(py)?;
                *self = Coa::Shared(pyref.clone_ref(py));
                Ok(pyref)
            }
        }
    }
}

impl<T> Coa<T>
where
    T: Convert + Extract + Clone,
    <T as Convert>::Output: PyClass,
{
    fn to_owned_class(&self, py: Python) -> PyResult<T> {
        match self {
            Coa::Owned(value) => Ok(value.clone()),
            Coa::Shared(pyref) => Extract::extract(py, pyref.clone_ref(py)),
        }
    }
}

impl<T> Coa<T>
where
    T: Convert + Extract + Clone,
    <T as Convert>::Output: PyTypeInfo + PyNativeType,
{
    fn to_owned_native(&self, py: Python) -> PyResult<T> {
        match self {
            Coa::Owned(value) => Ok(value.clone()),
            Coa::Shared(pyref) => Extract::extract(py, pyref.clone_ref(py)),
        }
    }
}

impl<T: Convert + Default> Default for Coa<T> {
    fn default() -> Self {
        Coa::Owned(T::default())
    }
}

impl<T: Convert> From<T> for Coa<T> {
    fn from(value: T) -> Self {
        Coa::Owned(value)
    }
}

// -<--------------------------------------------------------------------------

/// A single GenBank record.
#[pyclass(module = "gb_io")]
#[derive(Debug, Clone)]
pub struct Record {
    /// `str` or `None`: The name of the locus.
    #[pyo3(get, set)]
    name: Option<String>,
    /// `int` or `None`: The number of positions in the record sequence.
    #[pyo3(get, set)]
    len: Option<usize>,
    /// `str` or `None`: The type of molecule (DNA, RNA, etc.).
    #[pyo3(get, set)]
    molecule_type: Option<String>,
    /// `str`: The GenBank division to which the record belongs.
    #[pyo3(get, set)]
    division: String,
    /// `str` or `None`: The definition of the record.
    #[pyo3(get, set)]
    definition: Option<String>,
    /// `str` or `None`: The accession of the record.
    #[pyo3(get, set)]
    accession: Option<String>,
    /// `str` or `None`: The version of the record.
    #[pyo3(get, set)]
    version: Option<String>,
    /// `str` or `None`: The database link for the record.
    #[pyo3(get, set)]
    dblink: Option<String>,
    /// `str` or `None`: Word or phrase describing the sequence.
    #[pyo3(get, set)]
    keywords: Option<String>,

    topology: Topology,
    date: Option<Coa<gb_io::seq::Date>>,
    source: Option<Coa<gb_io::seq::Source>>,
    references: Coa<Vec<gb_io::seq::Reference>>,
    comments: Vec<String>,
    sequence: Vec<u8>,
    contig: Option<Coa<gb_io::seq::Location>>,
    features: Coa<Vec<gb_io::seq::Feature>>,
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

    //     let record = gb_io::seq::Seq {
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
    //     }.convert(py);
    //     Ok(record.into())
    // }

    /// `bool`: Whether the record described a circular molecule.
    #[getter]
    fn get_circular(slf: PyRef<'_, Self>) -> bool {
        match &slf.topology {
            Topology::Linear => false,
            Topology::Circular => true,
        }
    }

    #[setter]
    fn set_circular(mut slf: PyRefMut<'_, Self>, circular: bool) {
        if circular {
            slf.topology = Topology::Circular;
        } else {
            slf.topology = Topology::Linear;
        }
    }

    /// `~datetime.date` or `None`: The date this record was submitted.
    #[getter]
    fn get_date(mut slf: PyRefMut<'_, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        match &mut slf.deref_mut().date {
            Some(date) => Ok(date.to_shared(py)?.to_object(py)),
            None => Ok(py.None()),
        }
    }

    #[setter]
    fn set_date(mut slf: PyRefMut<'_, Self>, date: Option<&PyDate>) -> PyResult<()> {
        if let Some(dt) = date {
            slf.date = Some(Coa::Shared(Py::from(dt)));
        } else {
            slf.date = None;
        }
        Ok(())
    }

    /// `bytes`: The sequence of the record in lowercase, as raw ASCII.
    #[getter]
    fn get_sequence(slf: PyRef<'_, Self>) -> PyResult<PyObject> {
        // let seq = slf.seq.read().expect("failed to read lock");
        Ok(PyBytes::new(slf.py(), &slf.sequence).into())
    }

    /// `list`: A list of `Feature` within the record.
    #[getter]
    fn get_features(mut slf: PyRefMut<'_, Self>) -> PyResult<Py<PyList>> {
        let py = slf.py();
        slf.deref_mut().features.to_shared(py)
    }
}

impl Convert for gb_io::seq::Seq {
    type Output = Record;
    fn convert_with(self, py: Python, _interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        Py::new(
            py,
            Record {
                name: self.name,
                topology: self.topology,
                date: self.date.map(Coa::Owned),
                len: self.len,
                molecule_type: self.molecule_type,
                division: self.division,
                definition: self.definition,
                accession: self.accession,
                version: self.version,
                source: self.source.map(Coa::Owned),
                dblink: self.dblink,
                keywords: self.keywords,
                references: self.references.into(),
                comments: self.comments,
                sequence: self.seq,
                contig: self.contig.map(Coa::Owned),
                features: self.features.into(),
            },
        )
    }
}

impl Extract for gb_io::seq::Seq {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let record = object.as_ref(py).borrow();
        Ok(gb_io::seq::Seq {
            name: record.name.clone(),
            topology: record.topology.clone(),
            len: record.len.clone(),
            molecule_type: record.molecule_type.clone(),
            division: record.division.clone(),
            definition: record.definition.clone(),
            accession: record.accession.clone(),
            version: record.version.clone(),
            dblink: record.dblink.clone(),
            keywords: record.keywords.clone(),
            comments: record.comments.clone(),
            seq: record.sequence.clone(),
            references: record.references.to_owned_native(py)?,
            features: record.features.to_owned_native(py)?,
            date: record
                .date
                .as_ref()
                .map(|date| date.to_owned_native(py))
                .transpose()?,
            source: record
                .source
                .as_ref()
                .map(|source| source.to_owned_class(py))
                .transpose()?,
            contig: record
                .contig
                .as_ref()
                .map(|contig| contig.to_owned_native(py))
                .transpose()?,
        })
    }
}

// ---------------------------------------------------------------------------

/// The source of a GenBank record.
#[pyclass(module = "gb_io")]
#[derive(Debug, Default)]
pub struct Source {
    /// `str`: The name of the source organism.
    #[pyo3(get, set)]
    name: String,
    /// `str` or `None`: The scientific classification of the source organism.
    #[pyo3(get, set)]
    organism: Option<String>,
}

#[pymethods]
impl Source {
    #[new]
    #[pyo3(signature = (name, organism = None))]
    fn __new__(name: String, organism: Option<String>) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Self { name, organism })
    }

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<&PyAny> {
        let py = slf.py();
        let name = &slf.name;
        if let Some(v) = &slf.organism {
            PyString::new(py, "Source({}, {})").call_method1("format", (name, v))
        } else {
            PyString::new(py, "Source({})").call_method1("format", (name,))
        }
    }
}

impl Temporary for gb_io::seq::Source {
    fn temporary() -> Self {
        gb_io::seq::Source {
            source: String::new(),
            organism: None,
        }
    }
}

impl Convert for gb_io::seq::Source {
    type Output = Source;
    fn convert_with(self, py: Python, _interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        Py::new(
            py,
            Source {
                name: self.source,
                organism: self.organism,
            },
        )
    }
}

impl Extract for gb_io::seq::Source {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let source = object.extract::<&PyCell<Source>>(py)?.borrow();
        Ok(gb_io::seq::Source {
            source: source.name.clone(),
            organism: source.organism.clone(),
        })
    }
}

// ---------------------------------------------------------------------------

impl Convert for gb_io::seq::Date {
    type Output = PyDate;
    fn convert_with(self, py: Python, _interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        Ok(PyDate::new(py, self.year() as i32, self.month() as u8, self.day() as u8)?.into())
    }
}

impl Extract for gb_io::seq::Date {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let date = object.extract::<&PyDate>(py)?;
        Self::from_ymd(
            date.get_year(),
            date.get_month() as u32,
            date.get_day() as u32,
        )
        .map_err(|_| PyValueError::new_err("invalid date"))
    }
}

// ---------------------------------------------------------------------------

#[pyclass(module = "gb_io")]
#[derive(Debug, Clone)]
pub struct Feature {
    kind: Coa<gb_io::seq::FeatureKind>,
    location: Coa<gb_io::seq::Location>,
    qualifiers: Coa<Vec<(gb_io::QualifierKey, Option<String>)>>,
}

#[pymethods]
impl Feature {
    #[getter]
    fn get_kind<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<Py<PyString>> {
        let py = slf.py();
        slf.kind.to_shared(py)
    }

    #[setter]
    fn set_kind<'py>(mut slf: PyRefMut<'py, Self>, kind: &'py PyString) {
        slf.kind = Coa::Shared(Py::from(kind));
    }

    #[getter]
    fn get_location<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        slf.location.to_shared(py)
    }

    #[getter]
    fn get_qualifiers<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<Py<PyList>> {
        let py = slf.py();
        slf.qualifiers.to_shared(py)
    }
}

impl Convert for gb_io::seq::Feature {
    type Output = Feature;
    fn convert_with(self, py: Python, _interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        Py::new(
            py,
            Feature {
                kind: self.kind.into(),
                location: self.location.into(),
                qualifiers: self.qualifiers.into(),
            },
        )
    }
}

impl Extract for gb_io::seq::Feature {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let cell = object.as_ref(py);
        let feature = cell.borrow();
        Ok(gb_io::seq::Feature {
            kind: feature.kind.to_owned_native(py)?,
            location: feature.location.to_owned_native(py)?,
            qualifiers: Vec::new(),
        })
    }
}

impl Convert for gb_io::seq::FeatureKind {
    type Output = PyString;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        Ok(interner.intern(py, self.as_ref()))
    }
}

impl Extract for gb_io::seq::FeatureKind {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let s = object.extract::<&PyString>(py)?.to_str()?;
        Ok(gb_io::seq::FeatureKind::from(s))
    }
}

// ---------------------------------------------------------------------------

#[pyclass(module = "gb_io")]
#[derive(Debug)]
pub struct Qualifier {
    key: Coa<gb_io::QualifierKey>,
    #[pyo3(get, set)]
    value: Option<String>,
}

#[pymethods]
impl Qualifier {
    #[new]
    #[pyo3(signature = (key, value = None))]
    fn __new__(key: &PyString, value: Option<String>) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Self {
            key: Coa::Shared(Py::from(key)),
            value,
        })
    }

    fn __repr__<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<&PyAny> {
        let py = slf.py();
        let key = slf.key.to_shared(py)?;
        if let Some(v) = &slf.value {
            PyString::new(py, "Qualifier({}, {})").call_method1("format", (key, v))
        } else {
            PyString::new(py, "Qualifier({})").call_method1("format", (key,))
        }
    }

    #[getter]
    fn get_key<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<Py<PyString>> {
        let py = slf.py();
        slf.key.to_shared(py)
    }

    #[setter]
    fn set_key<'py>(mut slf: PyRefMut<'py, Self>, key: &'py PyString) {
        slf.key = Coa::Shared(Py::from(key));
    }
}

impl Convert for gb_io::QualifierKey {
    type Output = PyString;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        Ok(interner.intern(py, self))
    }
}

impl Extract for gb_io::QualifierKey {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let s = object.extract::<&PyString>(py)?.to_str()?;
        Ok(gb_io::QualifierKey::from(s))
    }
}

impl Convert for (gb_io::QualifierKey, Option<String>) {
    type Output = Qualifier;
    fn convert_with(self, py: Python, _interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        Py::new(
            py,
            Qualifier {
                key: self.0.into(),
                value: self.1,
            },
        )
    }
}

// ---------------------------------------------------------------------------

#[pyclass(module = "gb_io", subclass)]
#[derive(Debug)]
pub struct Location;

impl Convert for gb_io::seq::Location {
    type Output = PyAny;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        macro_rules! convert_vec {
            ($ty:ident, $inner:expr) => {{
                let objects: PyObject = $inner
                    .into_iter()
                    .map(|loc| loc.convert_with(py, interner))
                    .collect::<PyResult<Vec<PyObject>>>()
                    .map(|objects| PyList::new(py, objects))
                    .map(|list| list.to_object(py))?;
                Join::__new__(py, objects)
                    .and_then(|x| Py::new(py, x))
                    .map(|x| x.to_object(py))
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

impl Extract for gb_io::seq::Location {
    fn extract(py: Python, object: PyObject) -> PyResult<Self> {
        if let Ok(range) = object.downcast::<PyCell<Range>>(py) {
            let range = range.borrow();
            Ok(SeqLocation::Range(
                (range.start, gb_io::seq::Before(range.before)),
                (range.end, gb_io::seq::After(range.after)),
            ))
        } else if let Ok(between) = object.downcast::<PyCell<Between>>(py) {
            let between = between.borrow();
            Ok(SeqLocation::Between(between.start, between.end))
        } else if let Ok(complement) = object.downcast::<PyCell<Complement>>(py) {
            let location = Extract::extract(py, complement.borrow().location.clone_ref(py))?;
            Ok(SeqLocation::Complement(Box::new(location)))
        } else if let Ok(join) = object.downcast::<PyCell<Join>>(py) {
            let locations = Extract::extract(py, join.borrow().locations.clone_ref(py))?;
            Ok(SeqLocation::Join(locations))
        } else if let Ok(order) = object.downcast::<PyCell<Order>>(py) {
            let locations = Extract::extract(py, order.borrow().locations.clone_ref(py))?;
            Ok(SeqLocation::Order(locations))
        } else if let Ok(bond) = object.downcast::<PyCell<Bond>>(py) {
            let locations = Extract::extract(py, bond.borrow().locations.clone_ref(py))?;
            Ok(SeqLocation::Bond(locations))
        } else if let Ok(one_of) = object.downcast::<PyCell<OneOf>>(py) {
            let locations = Extract::extract(py, one_of.borrow().locations.clone_ref(py))?;
            Ok(SeqLocation::OneOf(locations))
        } else if let Ok(external) = object.downcast::<PyCell<External>>(py) {
            let external = external.borrow();
            let location = external
                .location
                .as_ref()
                .map(|loc| Extract::extract(py, loc.clone_ref(py)))
                .transpose()?
                .map(Box::new);
            Ok(SeqLocation::External(external.accession.clone(), location))
        } else {
            Err(PyTypeError::new_err("expected Location"))
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
    locations: Py<PyList>,
}

#[pymethods]
impl Join {
    #[new]
    fn __new__(py: Python, locations: PyObject) -> PyResult<PyClassInitializer<Self>> {
        let list = PyList::empty(py);
        for result in locations.as_ref(py).iter()? {
            let object = result?;
            object.downcast::<PyCell<Location>>()?;
            list.append(object)?;
        }
        Ok(PyClassInitializer::from(Location).add_subclass(Self {
            locations: Py::from(list),
        }))
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
        for obj in slf.locations.as_ref(py) {
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
        for obj in slf.locations.as_ref(py) {
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
    locations: Py<PyList>,
}

#[pymethods]
impl Order {
    #[new]
    fn __new__(py: Python, locations: PyObject) -> PyResult<PyClassInitializer<Self>> {
        let list = PyList::empty(py);
        for result in locations.as_ref(py).iter()? {
            let object = result?;
            object.downcast::<PyCell<Location>>()?;
            list.append(object)?;
        }
        Ok(PyClassInitializer::from(Location).add_subclass(Self {
            locations: Py::from(list),
        }))
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
    locations: Py<PyList>,
}

#[pymethods]
impl Bond {
    #[new]
    fn __new__(py: Python, locations: PyObject) -> PyResult<PyClassInitializer<Self>> {
        let list = PyList::empty(py);
        for result in locations.as_ref(py).iter()? {
            let object = result?;
            object.downcast::<PyCell<Location>>()?;
            list.append(object)?;
        }
        Ok(PyClassInitializer::from(Location).add_subclass(Self {
            locations: Py::from(list),
        }))
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
    locations: Py<PyList>,
}

#[pymethods]
impl OneOf {
    #[new]
    fn __new__(py: Python, locations: PyObject) -> PyResult<PyClassInitializer<Self>> {
        let list = PyList::empty(py);
        for result in locations.as_ref(py).iter()? {
            let object = result?;
            object.downcast::<PyCell<Location>>()?;
            list.append(object)?;
        }
        Ok(PyClassInitializer::from(Location).add_subclass(Self {
            locations: Py::from(list),
        }))
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

#[pyclass(module = "gb_io")]
pub struct Reference {
    #[pyo3(get, set)]
    description: String,
    #[pyo3(get, set)]
    title: String,
    #[pyo3(get, set)]
    authors: Option<String>,
    #[pyo3(get, set)]
    consortium: Option<String>,
    #[pyo3(get, set)]
    journal: Option<String>,
    #[pyo3(get, set)]
    pubmed: Option<String>,
    #[pyo3(get, set)]
    remark: Option<String>,
}

impl Convert for gb_io::seq::Reference {
    type Output = Reference;
    fn convert_with(self, py: Python, _interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        Py::new(
            py,
            Reference {
                description: self.description,
                authors: self.authors,
                consortium: self.consortium,
                title: self.title,
                journal: self.journal,
                pubmed: self.pubmed,
                remark: self.remark,
            },
        )
    }
}

impl Extract for gb_io::seq::Reference {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let reference = object.as_ref(py).borrow();
        Ok(gb_io::seq::Reference {
            description: reference.description.clone(),
            authors: reference.authors.clone(),
            consortium: reference.consortium.clone(),
            title: reference.title.clone(),
            journal: reference.journal.clone(),
            pubmed: reference.pubmed.clone(),
            remark: reference.remark.clone(),
        })
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
    m.add_class::<self::Qualifier>()?;
    m.add_class::<self::Feature>()?;
    m.add_class::<self::Record>()?;
    m.add_class::<self::RecordReader>()?;
    m.add_class::<self::Reference>()?;
    m.add_class::<self::Source>()?;
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
    ///     fh (`str` or file-handle): The path to a GenBank file, or a stream
    ///         that contains data serialized in GenBank format.
    ///
    /// Keywords Arguments:
    ///     escape_locus (`bool`): Pass `True` to escape any whitespace in
    ///         the locus name with an underscore character.
    ///     truncate_locus (`bool`): Pass `True` to trim the locus fields
    ///          so that the locus line is no longer than 79 characters.
    ///
    /// .. versionadded:: 0.2.0
    #[pyfn(m)]
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
            let seq = Extract::extract(py, record)?;
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
