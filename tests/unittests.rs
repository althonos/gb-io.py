extern crate gb_io_py;
extern crate lazy_static;
extern crate pyo3;

use std::sync::Mutex;

use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::types::PyList;

lazy_static::lazy_static! {
    pub static ref LOCK: Mutex<()> = Mutex::new(());
}

macro_rules! unittest {
    ($name:ident) => {
        #[test]
        fn $name() -> PyResult<()> {
            // initialize
            Python::initialize();

            // acquire Python only one test at a time
            let success = {
                let _l = LOCK.lock().unwrap();
                Python::attach(|py| {
                    // create a Python module from our rust code with debug symbols
                    let module = PyModule::new(py, "gb_io").unwrap();
                    gb_io_py::init(py, &module).unwrap();
                    py.import("sys")?
                        .getattr("modules")?
                        .cast::<PyDict>()?
                        .set_item("gb_io", &module)?;
                    // patch `sys.path` to locate tests from the project folder
                    py.import("sys")?
                        .getattr("path")?
                        .cast::<PyList>()?
                        .insert(0, env!("CARGO_MANIFEST_DIR"))?;
                    // run tests with the unittest runner
                    let kwargs = PyDict::new(py);
                    kwargs.set_item("verbosity", 2)?;
                    kwargs.set_item("exit", false)?;
                    let prog = py.import("unittest")?.call_method(
                        "main",
                        (concat!("tests.", stringify!($name)),),
                        Some(&kwargs),
                    )?;
                    // check run was was successful
                    prog.getattr("result")?
                        .call_method0("wasSuccessful")?
                        .extract::<bool>()
                })
            }?;

            // check the test succeeded
            if !success {
                panic!("unittest.main failed")
            }

            Ok(())
        }
    };
}

unittest!(test_doctests);
unittest!(test_biopython);
unittest!(test_load);
unittest!(test_dump);
unittest!(test_location);
unittest!(test_record);
