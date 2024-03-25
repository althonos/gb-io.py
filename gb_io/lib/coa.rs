use std::collections::HashMap;
use std::sync::RwLock;

use pyo3::prelude::*;
use pyo3::pyclass::PyClass;
use pyo3::types::PyByteArray;
use pyo3::types::PyList;
use pyo3::types::PyString;
use pyo3::PyNativeType;
use pyo3::PyTypeInfo;

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

/// A trait for types that can be converted to an equivalent Python type.
pub trait Convert: Sized {
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

impl Convert for Vec<u8> {
    type Output = PyByteArray;
    fn convert_with(self, py: Python, _interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        Ok(Py::from(PyByteArray::new(py, self.as_slice())))
    }
}

/// A trait for types that can be extracted from an equivalent Python type.
pub trait Extract: Convert {
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

impl Extract for Vec<u8> {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        Ok(object.as_ref(py).to_vec())
    }
}

// ---------------------------------------------------------------------------

/// A trait for obtaining a temporary value from a type.
pub trait Temporary: Sized {
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
pub enum Coa<T: Convert> {
    Owned(T),
    Shared(Py<<T as Convert>::Output>),
}

impl<T: Convert + Temporary> Coa<T> {
    pub fn to_shared(&mut self, py: Python) -> PyResult<Py<<T as Convert>::Output>> {
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
    pub fn to_owned_class(&self, py: Python) -> PyResult<T> {
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
    pub fn to_owned_native(&self, py: Python) -> PyResult<T> {
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
