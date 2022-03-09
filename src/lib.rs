extern crate gb_io;
extern crate pyo3;
extern crate pyo3_built;

mod built;
mod iter;
mod pyfile;

use std::io::Read;
use std::pin::Pin;

use gb_io::reader::SeqReader;
use gb_io::seq::Seq;
use gb_io::seq::Topology;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyList;
use pyo3::types::PyString;
use pyo3_built::pyo3_built;

use self::iter::RecordReader;
use self::pyfile::PyFileRead;

// ---------------------------------------------------------------------------

#[pyclass(module = "gb_io")]
#[derive(Debug, PartialEq)]
pub struct Record {
    seq: Seq,
}

#[pymethods]
impl Record {
    /// `str`, optional: The name of the record, or `None`.
    #[getter]
    fn get_name(&self) -> PyResult<Option<&str>> {
        Ok(self.seq.name.as_ref().map(String::as_str))
    }

    #[setter]
    fn set_name(&mut self, name: Option<String>) -> PyResult<()> {
        self.seq.name = name;
        Ok(())
    }

    /// `str`: The topology of the record.
    #[getter]
    fn get_topology(&self) -> PyResult<&str> {
        match self.seq.topology {
            Topology::Linear => Ok("linear"),
            Topology::Circular => Ok("circular"),
        }
    }

    #[setter]
    fn set_topology(&mut self, topology: &str) -> PyResult<()> {
        match topology {
            "linear" => {
                self.seq.topology = Topology::Linear;
                Ok(())
            }
            "circular" => {
                self.seq.topology = Topology::Circular;
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
    fn get_definition(&self) -> PyResult<Option<&str>> {
        Ok(self.seq.definition.as_ref().map(String::as_str))
    }

    #[setter]
    fn set_definition(&mut self, definition: Option<String>) -> PyResult<()> {
        self.seq.definition = definition;
        Ok(())
    }

    /// `str`, optional: The accession of the record, or `None`.
    #[getter]
    fn get_accession(&self) -> PyResult<Option<&str>> {
        Ok(self.seq.accession.as_ref().map(String::as_str))
    }

    #[setter]
    fn set_accession(&mut self, accession: Option<String>) -> PyResult<()> {
        self.seq.accession = accession;
        Ok(())
    }

    /// `str`, optional: The version of the record, or `None`.
    #[getter]
    fn get_version(&self) -> PyResult<Option<&str>> {
        Ok(self.seq.version.as_ref().map(String::as_str))
    }

    #[setter]
    fn set_version(&mut self, version: Option<String>) -> PyResult<()> {
        self.seq.version = version;
        Ok(())
    }

    // TODO: date, len, molecule_type, division,
    //       source, dblink, keywords,
    //       references, comments, contig, sequence, features

    #[getter]
    fn get_sequence(&self) -> PyResult<PyObject> {
        let gil = Python::acquire_gil();
        Ok(PyBytes::new(gil.python(), &self.seq.seq).into())
    }
}

impl From<Seq> for Record {
    fn from(seq: Seq) -> Self {
        Self { seq: seq }
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
        let path: Option<String>;
        let stream: Box<dyn Read> = if let Ok(s) = fh.cast_as::<PyString>() {
            // get a buffered reader to the resources pointed by `path`
            let bf = match std::fs::File::open(s.to_str()?) {
                Ok(f) => f,
                Err(e) => unimplemented!("error management"),
            };
            // store the path for later
            path = Some(s.to_str()?.to_string());
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
                    unimplemented!("error management")
                    // raise!(py, PyTypeError("expected path or binary file handle") from e)
                }
            };
            // extract the path from the `name` attribute
            path = fh
                .getattr("name")
                .and_then(|n| n.downcast::<PyString>().map_err(PyErr::from))
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
                .ok();
            // use a sequential or a threaded reader depending on `threads`.
            Box::new(bf)
        };

        // create the reader
        let mut reader = SeqReader::new(stream);

        // parse all records
        let mut records = PyList::empty(py);
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
