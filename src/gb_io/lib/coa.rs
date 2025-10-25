use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::RwLock;

use pyo3::prelude::*;
use pyo3::pyclass::PyClass;
use pyo3::types::PyByteArray;
use pyo3::types::PyList;
use pyo3::types::PyString;
use pyo3::PyTypeInfo;

use super::FeatureKind;
use super::QualifierKey;

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
    fn convert_bound_with<'py>(
        self,
        py: Python<'py>,
        interner: &mut PyInterner,
    ) -> PyResult<Bound<'py, Self::Output>>;
    fn convert_with(self, py: Python, interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        self.convert_bound_with(py, interner).map(|b| b.unbind())
    }
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
        Ok(Py::from(PyList::new(py, converted)?))
    }
    fn convert_bound_with<'py>(
        self,
        py: Python<'py>,
        interner: &mut PyInterner,
    ) -> PyResult<Bound<'py, Self::Output>> {
        let converted = self
            .into_iter()
            .map(|elem| elem.convert_bound_with(py, interner))
            .collect::<Result<Vec<_>, _>>()?;
        PyList::new(py, converted)
    }
}

impl Convert for Vec<u8> {
    type Output = PyByteArray;
    fn convert_with(self, py: Python, _interner: &mut PyInterner) -> PyResult<Py<Self::Output>> {
        Ok(Py::from(PyByteArray::new(py, self.as_slice())))
    }
    fn convert_bound_with<'py>(
        self,
        py: Python<'py>,
        _interner: &mut PyInterner,
    ) -> PyResult<Bound<'py, Self::Output>> {
        Ok(PyByteArray::new(py, self.as_slice()))
    }
}

/// A trait for types that can be extracted from an equivalent Python type.
pub trait Extract: Convert + 'static {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self>;
}

impl<T: Extract> Extract for Vec<T>
where
    Py<<T as Convert>::Output>: for<'a, 'py> FromPyObject<'a, 'py>,
    for<'a, 'py> PyErr: From<<Py<<T as Convert>::Output> as FromPyObject<'a, 'py>>::Error>,
    // PyErr: From<<pyo3::Py<<T as Convert>::Output> as pyo3::FromPyObject<'a, 'py>>::Error>
{
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        let list = object.bind(py);
        list.into_iter()
            .map(|elem| {
                let p = elem.unbind();
                let object: Py<<T as Convert>::Output> = p.extract(py).map_err(PyErr::from)?;
                <T as Extract>::extract(py, object)
            })
            .collect()
    }
}

impl Extract for Vec<u8> {
    fn extract(py: Python, object: Py<<Self as Convert>::Output>) -> PyResult<Self> {
        Ok(object.bind(py).to_vec())
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

impl Temporary for QualifierKey {
    fn temporary() -> Self {
        QualifierKey(Cow::Borrowed("gene"))
    }
}

impl Temporary for FeatureKind {
    fn temporary() -> Self {
        FeatureKind(Cow::Borrowed("locus_tag"))
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

#[derive(Debug)]
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

impl<T: Convert + Temporary + Clone> Clone for Coa<T> {
    fn clone(&self) -> Self {
        match self {
            Coa::Owned(c) => Coa::Owned(c.clone()),
            Coa::Shared(t) => Coa::Shared(t.clone()),
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
    <T as Convert>::Output: PyTypeInfo,
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
