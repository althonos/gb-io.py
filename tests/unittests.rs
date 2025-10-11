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
        fn $name() {
            // initialize
            Python::initialize();

            // acquire Python only one test at a time
            let success = {
                let _l = LOCK.lock().unwrap();
                Python::attach(|py| {
                    // create a Python module from our rust code with debug symbols
                    let module = PyModule::new(py, "gb_io").unwrap();
                    gb_io_py::init(py, &module).unwrap();
                    py.import("sys")
                        .unwrap()
                        .getattr("modules")
                        .unwrap()
                        .downcast::<PyDict>()
                        .unwrap()
                        .set_item("gb_io", &module)
                        .unwrap();
                    // patch `sys.path` to locate tests from the project folder
                    py.import("sys")
                        .unwrap()
                        .getattr("path")
                        .unwrap()
                        .downcast::<PyList>()
                        .unwrap()
                        .insert(0, env!("CARGO_MANIFEST_DIR"))
                        .unwrap();
                    // run tests with the unittest runner
                    let kwargs = PyDict::new(py);
                    kwargs.set_item("verbosity", 2).unwrap();
                    kwargs.set_item("exit", false).unwrap();
                    let prog = py
                        .import("unittest")
                        .unwrap()
                        .call_method(
                            "main",
                            (concat!("tests.", stringify!($name)),),
                            Some(&kwargs),
                        )
                        .unwrap();
                    // check run was was successful
                    prog.getattr("result")
                        .unwrap()
                        .call_method0("wasSuccessful")
                        .unwrap()
                        .extract::<bool>()
                        .unwrap()
                })
            };

            // check the test succeeded
            if !success {
                panic!("unittest.main failed")
            }
        }
    };
}

unittest!(test_doctests);
unittest!(test_biopython);
unittest!(test_load);
unittest!(test_dump);
unittest!(test_location);
