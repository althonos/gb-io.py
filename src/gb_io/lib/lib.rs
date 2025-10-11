extern crate gb_io;
extern crate libc;
extern crate pyo3;
extern crate pyo3_built;

mod built;
mod coa;
mod pyfile;
mod reader;

use std::borrow::Cow;
use std::convert::Infallible;
use std::io::Read;
use std::io::Write;
use std::ops::DerefMut;

use gb_io::reader::GbParserError;
use gb_io::reader::SeqReader;
use gb_io::seq::After;
use gb_io::seq::Before;
use gb_io::seq::Location as SeqLocation;
use gb_io::seq::Topology;
use gb_io::writer::SeqWriter;
use pyo3::conversion::IntoPyObjectExt;
use pyo3::exceptions::PyIOError;
use pyo3::exceptions::PyNotImplementedError;
use pyo3::exceptions::PyOSError;
use pyo3::exceptions::PyTypeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyByteArray;
use pyo3::types::PyDate;
// use pyo3::types::PyDateAccess;
use pyo3::types::PyIterator;
use pyo3::types::PyList;
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use pyo3::types::PyTuple;
use pyo3_built::pyo3_built;

use self::coa::Coa;
use self::coa::Convert;
use self::coa::Extract;
use self::coa::PyInterner;
use self::coa::Temporary;
use self::pyfile::PyFileRead;
use self::pyfile::PyFileWrite;
use self::reader::RecordReader;

// ---------------------------------------------------------------------------

/// A single GenBank record.
#[pyclass(module = "gb_io")]
#[derive(Debug, Clone)]
pub struct Record {
    /// `str` or `None`: The name of the locus.
    #[pyo3(get, set)]
    name: Option<String>,
    /// `int` or `None`: The number of positions in the record sequence.
    #[pyo3(get, set)]
    length: Option<usize>,
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
    date: Option<gb_io::seq::Date>,
    source: Option<Coa<gb_io::seq::Source>>,
    references: Coa<Vec<gb_io::seq::Reference>>,
    comments: Vec<String>,
    sequence: Coa<Vec<u8>>,
    contig: Option<Coa<gb_io::seq::Location>>,
    features: Coa<Vec<gb_io::seq::Feature>>,
}

impl Default for Record {
    fn default() -> Self {
        Record {
            name: None,
            length: None,
            molecule_type: None,
            division: String::from("UNK"),
            definition: None,
            accession: None,
            version: None,
            dblink: None,
            keywords: None,
            topology: Topology::Linear,
            date: None,
            source: None,
            references: Coa::Owned(Vec::new()),
            comments: Vec::new(),
            sequence: Coa::Owned(Vec::new()),
            contig: None,
            features: Coa::Owned(Vec::new()),
        }
    }
}

#[pymethods]
impl Record {
    /// Create a new record.
    #[new]
    #[pyo3(signature = (
        sequence,
        *,
        name = None,
        length = None,
        molecule_type = None,
        division = String::from("UNK"),
        definition = None,
        accession = None,
        version = None,
        dblink = None,
        keywords = None,
        circular = false,
        date = None,
        source = None,
        contig = None,
        references = None,
        features = None,
    ))]
    fn __new__<'py>(
        sequence: &Bound<'py, PyAny>,
        name: Option<String>,
        length: Option<usize>,
        molecule_type: Option<String>,
        division: String,
        definition: Option<String>,
        accession: Option<String>,
        version: Option<String>,
        dblink: Option<String>,
        keywords: Option<String>,
        circular: bool,
        date: Option<Bound<'py, PyAny>>,
        source: Option<Py<Source>>,
        contig: Option<Py<Location>>,
        references: Option<Bound<'py, PyAny>>,
        features: Option<Bound<'py, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let py = sequence.py();
        let mut record = Record::default();
        record.name = name;
        record.length = length;
        record.molecule_type = molecule_type;
        record.division = division;
        record.definition = definition;
        record.accession = accession;
        record.version = version;
        record.dblink = dblink;
        record.keywords = keywords;
        record.source = source.map(|source| Coa::Shared(source.clone_ref(py)));
        record.contig = contig.map(|contig| Coa::Shared(contig.clone_ref(py)));
        record.sequence = PyByteArray::from(sequence).map(Py::from).map(Coa::Shared)?;

        if let Some(dt) = date {
            let year = dt.getattr("year")?.extract::<i32>()?;
            let month = dt.getattr("month")?.extract::<u32>()?;
            let day = dt.getattr("day")?.extract::<u32>()?;
            match gb_io::seq::Date::from_ymd(year, month as u32, day as u32) {
                Ok(dt) => record.date = Some(dt),
                Err(_) => return Err(PyValueError::new_err("invalid date")),
            }
        }

        if circular {
            record.topology = Topology::Circular;
        }
        if let Some(features_iter) = features {
            let feature_list = PyList::empty(py);
            for result in features_iter.try_iter()? {
                let object = result?;
                object.extract::<Bound<'py, Feature>>()?;
                feature_list.append(object)?;
            }
            record.features = Coa::Shared(Py::from(feature_list));
        }
        if let Some(reference_iter) = references {
            let reference_list = PyList::empty(py);
            for result in reference_iter.try_iter()? {
                let object = result?;
                object.extract::<Bound<Reference>>()?;
                reference_list.append(object)?;
            }
            record.references = Coa::Shared(Py::from(reference_list));
        }

        Ok(PyClassInitializer::from(record))
    }

    /// `bool`: Whether the record describes a circular molecule.
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
            Some(date) => {
                Ok(PyDate::new(py, date.year() as _, date.month() as _, date.day() as _)?.into())
                // date.to_shared(py)?.into_py_any(py),
            }
            None => Ok(py.None()),
        }
    }

    #[setter]
    fn set_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        date: Option<Bound<'py, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        if let Some(dt) = date {
            let year = dt.getattr("year")?.extract::<i32>()?;
            let month = dt.getattr("month")?.extract::<u32>()?;
            let day = dt.getattr("day")?.extract::<u32>()?;
            match gb_io::seq::Date::from_ymd(year, month as u32, day as u32) {
                Ok(dt) => slf.date = Some(dt),
                Err(_) => return Err(PyValueError::new_err("invalid date")),
            }
        } else {
            slf.date = None;
        }
        Ok(())
    }

    /// `bytes`: The sequence of the record in lowercase, as raw ASCII.
    #[getter]
    fn get_sequence(mut slf: PyRefMut<'_, Self>) -> PyResult<Py<PyByteArray>> {
        let py = slf.py();
        slf.sequence.to_shared(py)
    }

    #[setter]
    fn set_sequence(mut slf: PyRefMut<'_, Self>, sequence: Py<PyByteArray>) {
        slf.sequence = Coa::Shared(sequence);
    }

    /// `list`: A list of `Feature` within the record.
    #[getter]
    fn get_features(mut slf: PyRefMut<'_, Self>) -> PyResult<Py<PyList>> {
        let py = slf.py();
        slf.deref_mut().features.to_shared(py)
    }

    #[setter]
    fn set_features(mut slf: PyRefMut<'_, Self>, features: Py<PyList>) {
        slf.features = Coa::Shared(features);
    }

    /// `list`: A list of `Reference` within the record.
    #[getter]
    fn get_references(mut slf: PyRefMut<'_, Self>) -> PyResult<Py<PyList>> {
        let py = slf.py();
        slf.deref_mut().references.to_shared(py)
    }

    #[setter]
    fn set_references(mut slf: PyRefMut<'_, Self>, references: Py<PyList>) {
        slf.references = Coa::Shared(references);
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
                date: self.date,
                length: self.len,
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
                sequence: Coa::Owned(self.seq),
                contig: self.contig.map(Coa::Owned),
                features: self.features.into(),
            },
        )
    }
}

impl Extract for gb_io::seq::Seq {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let record = object.bind(py).borrow();
        Ok(gb_io::seq::Seq {
            name: record.name.clone(),
            topology: record.topology.clone(),
            len: record.length.clone(),
            molecule_type: record.molecule_type.clone(),
            division: record.division.clone(),
            definition: record.definition.clone(),
            accession: record.accession.clone(),
            version: record.version.clone(),
            dblink: record.dblink.clone(),
            keywords: record.keywords.clone(),
            comments: record.comments.clone(),
            seq: record.sequence.to_owned_native(py)?,
            references: record.references.to_owned_native(py)?,
            features: record.features.to_owned_native(py)?,
            date: record.date.clone(),
            source: record
                .source
                .as_ref()
                .map(|source| source.to_owned_class(py))
                .transpose()?,
            contig: record
                .contig
                .as_ref()
                .map(|contig| contig.to_owned_class(py))
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

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let name = &slf.name;
        if let Some(v) = &slf.organism {
            PyString::new(py, "Source({!r}, {!r})").call_method1("format", (name, v))
        } else {
            PyString::new(py, "Source({!r})").call_method1("format", (name,))
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
        let source = object.extract::<Bound<Source>>(py)?.borrow();
        Ok(gb_io::seq::Source {
            source: source.name.clone(),
            organism: source.organism.clone(),
        })
    }
}

// ---------------------------------------------------------------------------

/// A feature located somewhere in the record.
#[pyclass(module = "gb_io")]
#[derive(Debug, Clone)]
pub struct Feature {
    kind: Coa<FeatureKind>,
    location: Coa<gb_io::seq::Location>,
    qualifiers: Coa<Vec<(QualifierKey, Option<String>)>>,
}

#[pymethods]
impl Feature {
    #[new]
    #[pyo3(signature = (kind, location, qualifiers = None))]
    fn __new__(
        kind: Py<PyString>,
        location: Py<Location>,
        qualifiers: Option<Py<PyList>>,
    ) -> PyClassInitializer<Self> {
        let kind = Coa::Shared(kind);
        let location = Coa::Shared(location);
        let qualifiers = qualifiers.map(Coa::Shared).unwrap_or_default();
        PyClassInitializer::from(Self {
            kind,
            location,
            qualifiers,
        })
    }

    fn __repr__<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let kind = slf.kind.to_shared(py)?;
        let location = slf.location.to_shared(py)?;
        let qualifiers = slf.qualifiers.to_shared(py)?;
        if qualifiers.bind(py).is_empty() {
            PyString::new(py, "Feature(kind={!r}, location={!r})")
                .call_method1("format", (kind, location))
        } else {
            PyString::new(py, "Feature(kind={!r}, location={!r}, qualifiers={!r})")
                .call_method1("format", (kind, location, qualifiers))
        }
    }

    /// `str`: The kind of feature.
    #[getter]
    fn get_kind<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<Py<PyString>> {
        let py = slf.py();
        slf.kind.to_shared(py)
    }

    #[setter]
    fn set_kind<'py>(mut slf: PyRefMut<'py, Self>, kind: Bound<'py, PyString>) {
        slf.kind = Coa::Shared(kind.unbind());
    }

    /// `Location`: The location of the feature in the record.
    #[getter]
    fn get_location<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<Py<Location>> {
        let py = slf.py();
        slf.location.to_shared(py)
    }

    #[setter]
    fn set_location<'py>(mut slf: PyRefMut<'py, Self>, kind: Py<Location>) {
        slf.location = Coa::Shared(kind.clone_ref(slf.py()));
    }

    /// `list`: A list of `Qualifier` for this particular feature.
    #[getter]
    fn get_qualifiers<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<Py<PyList>> {
        let py = slf.py();
        slf.qualifiers.to_shared(py)
    }

    #[setter]
    fn set_qualifiers<'py>(mut slf: PyRefMut<'py, Self>, qualifiers: Py<PyList>) {
        slf.qualifiers = Coa::Shared(qualifiers.clone_ref(slf.py()));
    }
}

impl Convert for gb_io::seq::Feature {
    type Output = Feature;
    fn convert_with(self, py: Python, _interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        Py::new(
            py,
            Feature {
                kind: Coa::from(FeatureKind(self.kind)),
                location: self.location.into(),
                qualifiers: self
                    .qualifiers
                    .into_iter()
                    .map(|(x, y)| (QualifierKey(x), y))
                    .collect::<Vec<_>>()
                    .into(),
            },
        )
    }
}

impl Extract for gb_io::seq::Feature {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let cell = object.bind(py);
        let feature = cell.borrow();
        Ok(gb_io::seq::Feature {
            kind: feature.kind.to_owned_native(py)?.0,
            location: feature.location.to_owned_class(py)?,
            qualifiers: feature
                .qualifiers
                .to_owned_native(py)?
                .into_iter()
                .map(|(x, y)| (x.0, y))
                .collect(),
        })
    }
}

#[derive(Debug, Clone)]
struct FeatureKind(Cow<'static, str>);

impl Convert for FeatureKind {
    type Output = PyString;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        Ok(interner.intern(py, self.0.as_ref()))
    }
}

impl Extract for FeatureKind {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let s = object.extract::<Bound<PyString>>(py)?;
        Ok(FeatureKind(Cow::from(s.to_cow()?.into_owned())))
    }
}

// ---------------------------------------------------------------------------

/// A single key-value qualifier for a `Feature`.
#[pyclass(module = "gb_io")]
#[derive(Debug)]
pub struct Qualifier {
    key: Coa<QualifierKey>,
    /// `str` or `None`: An optional value for the qualifier.
    #[pyo3(get, set)]
    value: Option<String>,
}

#[pymethods]
impl Qualifier {
    #[new]
    #[pyo3(signature = (key, value = None))]
    fn __new__(key: Bound<PyString>, value: Option<String>) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Self {
            key: Coa::Shared(key.unbind()),
            value,
        })
    }

    fn __repr__<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let key = slf.key.to_shared(py)?;
        if let Some(v) = &slf.value {
            PyString::new(py, "Qualifier({!r}, {!r})").call_method1("format", (key, v))
        } else {
            PyString::new(py, "Qualifier({!r})").call_method1("format", (key,))
        }
    }

    /// `str`: The qualifier key.
    #[getter]
    fn get_key<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<Py<PyString>> {
        let py = slf.py();
        slf.key.to_shared(py)
    }

    #[setter]
    fn set_key<'py>(mut slf: PyRefMut<'py, Self>, key: Bound<'py, PyString>) {
        slf.key = Coa::Shared(key.unbind());
    }
}

#[derive(Debug, Clone)]
struct QualifierKey(Cow<'static, str>);

impl Convert for QualifierKey {
    type Output = PyString;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        Ok(interner.intern(py, self.0))
    }
}

impl Extract for QualifierKey {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let s = object.extract::<Bound<PyString>>(py)?;
        Ok(QualifierKey(Cow::from(s.to_cow()?.into_owned())))
    }
}

impl Convert for (QualifierKey, Option<String>) {
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

impl Extract for (QualifierKey, Option<String>) {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let py_cell = object.bind(py);
        let key = py_cell.borrow().key.to_owned_native(py)?;
        let value = py_cell.borrow().value.clone();
        Ok((key, value))
    }
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Strand {
    Direct,
    Reverse,
}

impl<'py> FromPyObject<'py> for Strand {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let py = ob.py();
        let value = ob.extract::<Bound<PyString>>()?;
        if value == "+" {
            Ok(Strand::Direct)
        } else if value == "-" {
            Ok(Strand::Reverse)
        } else {
            Err(PyValueError::new_err(
                PyString::new(py, "invalid strand: {!r}")
                    .call_method1("format", (value,))?
                    .into_py_any(py)?,
            ))
        }
    }
}

impl<'py> IntoPyObject<'py> for Strand {
    type Target = PyString;
    type Output = Bound<'py, PyString>;
    type Error = Infallible;
    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        match self {
            Strand::Direct => Ok(pyo3::intern!(py, "+").clone()),
            Strand::Reverse => Ok(pyo3::intern!(py, "-").clone()),
        }
    }
}

/// A base location for a `Feature`.
///
/// This class cannot be instantiated directly, and should not be derived
/// to avoid breakage in the Rust code. It can however be used for type
/// annotations where any concrete `Location` subclass can be given.
#[pyclass(module = "gb_io", subclass)]
#[derive(Debug)]
pub struct Location;

impl Convert for gb_io::seq::Location {
    type Output = Location;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        macro_rules! convert_vec {
            ($ty:ident, $inner:expr) => {{
                let objects: PyObject = $inner
                    .into_iter()
                    .map(|loc| loc.convert_with(py, interner))
                    .collect::<PyResult<Vec<Py<Location>>>>()
                    .map(|objects| PyList::new(py, objects))
                    .and_then(|list| list?.into_py_any(py)?.extract(py))?;
                Join::__new__(py, objects)
                    .and_then(|x| Py::new(py, x))
                    .and_then(|x| match x.into_py_any(py)?.extract::<Py<Location>>(py) {
                        Ok(pyref) => Ok(pyref.clone_ref(py)),
                        Err(e) => Err(PyErr::from(e)),
                    })
            }};
        }

        match self {
            SeqLocation::Range((start, Before(before)), (end, After(after))) => {
                Py::new(py, Range::__new__(start, end, before, after)).and_then(|x| {
                    match x.into_py_any(py)?.extract::<Py<Location>>(py) {
                        Ok(pyref) => Ok(pyref.clone_ref(py)),
                        Err(e) => Err(PyErr::from(e)),
                    }
                })
            }
            SeqLocation::Between(start, end) => {
                Py::new(py, Between::__new__(start, end)).and_then(|x| {
                    match x.into_py_any(py)?.extract::<Py<Location>>(py) {
                        Ok(pyref) => Ok(pyref.clone_ref(py)),
                        Err(e) => Err(PyErr::from(e)),
                    }
                })
            }
            SeqLocation::Complement(inner_location) => (*inner_location)
                .convert_with(py, interner)
                .and_then(|inner| Py::new(py, Complement::__new__(inner)))
                .and_then(|x| match x.into_py_any(py)?.extract::<Py<Location>>(py) {
                    Ok(pyref) => Ok(pyref.clone_ref(py)),
                    Err(e) => Err(PyErr::from(e)),
                }),
            SeqLocation::Join(inner_locations) => convert_vec!(Join, inner_locations),
            SeqLocation::Order(inner_locations) => convert_vec!(Order, inner_locations),
            SeqLocation::Bond(inner_locations) => convert_vec!(Bond, inner_locations),
            SeqLocation::OneOf(inner_locations) => convert_vec!(OneOf, inner_locations),
            SeqLocation::External(accession, location) => {
                let loc = location.map(|x| x.convert_with(py, interner)).transpose()?;
                Py::new(py, External::__new__(accession, loc)).and_then(|x| {
                    match x.into_py_any(py)?.extract::<Py<Location>>(py) {
                        Ok(pyref) => Ok(pyref.clone_ref(py)),
                        Err(e) => Err(PyErr::from(e)),
                    }
                })
            }
            _ => Err(PyNotImplementedError::new_err(format!(
                "conversion of {:?}",
                self
            ))),
        }
    }
}

impl Extract for gb_io::seq::Location {
    fn extract(py: Python, object: Py<Location>) -> PyResult<Self> {
        let location = object.bind(py);
        if let Ok(range) = location.extract::<Bound<Range>>() {
            let range = range.borrow();
            Ok(SeqLocation::Range(
                (range.start, gb_io::seq::Before(range.before)),
                (range.end, gb_io::seq::After(range.after)),
            ))
        } else if let Ok(between) = location.extract::<Bound<Between>>() {
            let between = between.borrow();
            Ok(SeqLocation::Between(between.start, between.end))
        } else if let Ok(complement) = location.extract::<Bound<Complement>>() {
            let location = Extract::extract(py, complement.borrow().location.clone_ref(py))?;
            Ok(SeqLocation::Complement(Box::new(location)))
        } else if let Ok(join) = location.extract::<Bound<Join>>() {
            let locations = Extract::extract(py, join.borrow().locations.clone_ref(py))?;
            Ok(SeqLocation::Join(locations))
        } else if let Ok(order) = location.extract::<Bound<Order>>() {
            let locations = Extract::extract(py, order.borrow().locations.clone_ref(py))?;
            Ok(SeqLocation::Order(locations))
        } else if let Ok(bond) = location.extract::<Bound<Bond>>() {
            let locations = Extract::extract(py, bond.borrow().locations.clone_ref(py))?;
            Ok(SeqLocation::Bond(locations))
        } else if let Ok(one_of) = location.extract::<Bound<OneOf>>() {
            let locations = Extract::extract(py, one_of.borrow().locations.clone_ref(py))?;
            Ok(SeqLocation::OneOf(locations))
        } else if let Ok(external) = location.extract::<Bound<External>>() {
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

/// A location for a `Feature` spanning over a range of consecutive positions.
///
/// The additional ``before`` and ``after`` flags can be set to indicate the
/// feature spans before its starting index and/or after its ending index.
/// For instance, a feature location of ``<1..206`` can be created with
/// ``Range(1, 206, before=True)``.
///
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

    #[getter]
    fn get_strand<'py>(slf: PyRef<'py, Self>) -> Bound<'py, PyString> {
        Strand::Direct.into_pyobject(slf.py()).unwrap()
    }
}

/// A location for a `Feature` located between two consecutive positions.
#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct Between {
    /// `int`: The start of the position interval.
    #[pyo3(get, set)]
    start: i64,
    /// `int`: The end of the position interval.
    #[pyo3(get, set)]
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

    #[getter]
    fn get_strand<'py>(slf: PyRef<'py, Self>) -> Bound<'py, PyString> {
        Strand::Direct.into_pyobject(slf.py()).unwrap()
    }
}

/// A location for a `Feature` on the opposite strand of a given `Location`.
#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct Complement {
    /// `Location`: The location on the complement strand.
    #[pyo3(get, set)]
    location: Py<Location>,
}

#[pymethods]
impl Complement {
    #[new]
    fn __new__(location: Py<Location>) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Location).add_subclass(Self { location })
    }

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        PyString::new(py, "Complement({!r})")
            .call_method1("format", (Py::clone_ref(&slf.location, py),))
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

    #[getter]
    fn get_strand<'py>(slf: PyRef<'py, Self>) -> PyResult<Bound<'py, PyString>> {
        let py = slf.py();
        match slf
            .location
            .getattr(py, "strand")
            .and_then(|start| start.extract(py))?
        {
            Strand::Direct => Strand::Reverse
                .into_pyobject(py)
                .map_err(|_| unreachable!()),
            Strand::Reverse => Strand::Direct.into_pyobject(py).map_err(|_| unreachable!()),
        }
    }
}

/// A location for a `Feature` consisting in joined sequence spans.
#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct Join {
    /// `list` of `Location`: The locations part of the joint location.
    #[pyo3(get, set)]
    locations: Py<PyList>,
}

#[pymethods]
impl Join {
    #[new]
    fn __new__(py: Python, locations: PyObject) -> PyResult<PyClassInitializer<Self>> {
        let list = PyList::empty(py);
        for result in locations.bind(py).try_iter()? {
            let object = result?;
            object.extract::<Bound<Location>>()?;
            list.append(object)?;
        }
        Ok(PyClassInitializer::from(Location).add_subclass(Self {
            locations: Py::from(list),
        }))
    }

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        PyString::new(py, "Join({!r})").call_method1("format", (&slf.locations,))
    }

    #[getter]
    fn get_start<'py>(slf: PyRef<'py, Self>) -> PyResult<i32> {
        let py = slf.py();
        let mut min: Option<i32> = None;
        for obj in slf.locations.bind(py) {
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
        for obj in slf.locations.bind(py) {
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

/// A location for a `Feature` over disjoint locations in the given order.
#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct Order {
    /// `list` of `Location`: The locations part of the ordered location.
    #[pyo3(get, set)]
    locations: Py<PyList>,
}

#[pymethods]
impl Order {
    #[new]
    fn __new__(py: Python, locations: PyObject) -> PyResult<PyClassInitializer<Self>> {
        let list = PyList::empty(py);
        for result in locations.bind(py).try_iter()? {
            let object = result?;
            object.extract::<Bound<Location>>()?;
            list.append(object)?;
        }
        Ok(PyClassInitializer::from(Location).add_subclass(Self {
            locations: Py::from(list),
        }))
    }

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        PyString::new(py, "Order({!r})").call_method1("format", (&slf.locations,))
    }
}

/// A location for a `Feature` corresponding to a bond between locations.
#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct Bond {
    #[pyo3(get, set)]
    locations: Py<PyList>,
}

#[pymethods]
impl Bond {
    #[new]
    fn __new__(py: Python, locations: PyObject) -> PyResult<PyClassInitializer<Self>> {
        let list = PyList::empty(py);
        for result in locations.bind(py).try_iter()? {
            let object = result?;
            object.extract::<Bound<Location>>()?;
            list.append(object)?;
        }
        Ok(PyClassInitializer::from(Location).add_subclass(Self {
            locations: Py::from(list),
        }))
    }

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        PyString::new(py, "Bond({!r})").call_method1("format", (&slf.locations,))
    }
}

/// A location for a `Feature` located at one of the given locations.
#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct OneOf {
    /// `list` of `Location`: The locations at one of which this feature is located.
    #[pyo3(get, set)]
    locations: Py<PyList>,
}

#[pymethods]
impl OneOf {
    #[new]
    fn __new__(py: Python, locations: PyObject) -> PyResult<PyClassInitializer<Self>> {
        let list = PyList::empty(py);
        for result in locations.bind(py).try_iter()? {
            let object = result?;
            object.extract::<Bound<Location>>()?;
            list.append(object)?;
        }
        Ok(PyClassInitializer::from(Location).add_subclass(Self {
            locations: Py::from(list),
        }))
    }

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        PyString::new(py, "OneOf({!r})").call_method1("format", (&slf.locations,))
    }
}

/// A location for a `Feature` located in an external record.
#[pyclass(module = "gb_io", extends = Location)]
#[derive(Debug)]
pub struct External {
    /// `str`: The accession of the external record where the feature is located.
    #[pyo3(get, set)]
    accession: String,
    /// `Location` or `None`: The location of the feature in the external record.
    #[pyo3(get, set)]
    location: Option<Py<Location>>,
}

#[pymethods]
impl External {
    #[new]
    #[pyo3(signature = (accession, location=None))]
    fn __new__(accession: String, location: Option<Py<Location>>) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Location).add_subclass(Self {
            accession,
            location,
        })
    }

    fn __repr__<'py>(slf: PyRef<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        match &slf.location {
            Some(s) => PyString::new(py, "External({!r}, {!r})")
                .call_method1("format", (&slf.accession, s)),
            None => PyString::new(py, "External({!r})").call_method1("format", (&slf.accession,)),
        }
    }
}

// ---------------------------------------------------------------------------

/// A reference for a record.
#[pyclass(module = "gb_io")]
pub struct Reference {
    /// `str`: The title of the publication.
    #[pyo3(get, set)]
    title: String,
    /// The record location described by the publication.
    #[pyo3(get, set)]
    description: String,
    /// `str` or `None`: The authors as they appear in the original publication.
    #[pyo3(get, set)]
    authors: Option<String>,
    /// `str` or `None`: The consortium behind the publication, if any.
    #[pyo3(get, set)]
    consortium: Option<String>,
    /// `str` or `None`: The journal where the reference was published.
    #[pyo3(get, set)]
    journal: Option<String>,
    /// `str` or `None`: A PubMed identifier for the publication, if any.
    #[pyo3(get, set)]
    pubmed: Option<String>,
    /// `str` or `None`: A remark about the reference.
    #[pyo3(get, set)]
    remark: Option<String>,
}

#[pymethods]
impl Reference {
    #[new]
    #[pyo3(signature = (title, description, authors=None, consortium=None, journal=None, pubmed=None, remark=None))]
    fn __new__(
        title: String,
        description: String,
        authors: Option<String>,
        consortium: Option<String>,
        journal: Option<String>,
        pubmed: Option<String>,
        remark: Option<String>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Self {
            title,
            description,
            authors,
            consortium,
            journal,
            pubmed,
            remark,
        })
    }
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
        let reference = object.bind(py).borrow();
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
/// Example:
///     Load all the GenBank records from a single file, and print their
///     accession::
///
///         >>> import gb_io
///         >>> records = gb_io.load("tests/data/AY048670.1.gb")
///         >>> print([record.accession for record in records])
///         ['AY048670']
///
///     Iterate over records inside a `gzip` compressed GenBank file, and
///     display the accession and sequence length of each record larger
///     than 400,000bp::
///
///         >>> import gb_io
///         >>> import gzip
///         >>> with gzip.open("tests/data/JAOQKG01.1.gb.gz", "rb") as reader:
///         ...     for record in gb_io.iter(reader):
///         ...         if len(record.sequence) > 400000:
///         ...             print(record.name, len(record.sequence))
///         JAOQKG010000001 754685
///         JAOQKG010000002 569365
///         JAOQKG010000003 418835
///         JAOQKG010000004 418347
///
///
#[pymodule]
#[pyo3(name = "lib")]
pub fn init(py: Python, m: &Bound<PyModule>) -> PyResult<()> {
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
    fn load(py: Python, fh: &Bound<PyAny>) -> PyResult<Py<PyList>> {
        // extract either a path or a file-handle from the arguments
        // let path: Option<String>;
        let stream: Box<dyn Read> = if let Ok(s) = fh.downcast::<PyString>() {
            // get a buffered reader to the resources pointed by `path`
            let bf = match std::fs::File::open(s.to_cow()?.as_ref()) {
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
            let bf = match PyFileRead::from_ref(fh.clone()) {
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
        Ok(records.unbind())
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
    fn iter(py: Python, fh: Bound<PyAny>) -> PyResult<Py<RecordReader>> {
        let reader = match fh.downcast::<PyString>() {
            Ok(s) => RecordReader::from_path(s.to_cow()?.as_ref())?,
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
    fn dump<'py>(
        py: Python<'py>,
        records: Bound<'py, PyAny>,
        fh: Bound<'py, PyAny>,
        escape_locus: bool,
        truncate_locus: bool,
    ) -> PyResult<()> {
        // extract either a path or a file-handle from the arguments
        let stream: Box<dyn Write> = if let Ok(s) = fh.downcast::<PyString>() {
            // get a buffered reader to the resources pointed by `path`
            let bf = match std::fs::File::create(s.to_cow()?.as_ref()) {
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
        let it = if let Ok(record) = records.extract::<Bound<'_, Record>>() {
            PyIterator::from_object(&PyTuple::new(py, [record])?.into_py_any(py)?.bind(py))?
        } else {
            PyIterator::from_object(&records)?
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
