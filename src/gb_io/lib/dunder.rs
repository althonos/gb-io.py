use pyo3::prelude::*;

pub trait PyRepr<'py> {
    type Output: IntoPyObject<'py>;
    type Error: Into<PyErr>;
    fn repr(&self, py: Python<'py>) -> Result<Self::Output, Self::Error>;
}

impl<'py, T> PyRepr<'py> for Bound<'py, T>
where
    T: PyRepr<'py> + pyo3::PyClass,
{
    type Output = <T as PyRepr<'py>>::Output;
    type Error = <T as PyRepr<'py>>::Error;
    fn repr(&self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        self.borrow().repr(py)
    }
}
