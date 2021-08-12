/*
 * Copyright 2021 Aon Cyber Solutions
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 */

use crate::err::PyRegError;
use crate::py_reg_key::PyRegKey;
use crate::py_reg_value::PyRegValue;
use crate::util::{init_logging, FileOrFileLike};
use notatin::{cell_key_node::CellKeyNode, parser::Parser};
use pyo3::exceptions::{PyNotImplementedError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::PyIterProtocol;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};

pub trait ReadSeek: Read + Seek {
    fn tell(&mut self) -> io::Result<u64> {
        self.seek(SeekFrom::Current(0))
    }
}

impl<T: Read + Seek> ReadSeek for T {}

#[pyclass(subclass)]
/// PyRegParser(self, path_or_file_like, /)
/// --
///
/// Returns an instance of the parser.
/// Works on both a path (string), or a file-like object.
pub struct PyRegParser {
    pub inner: Option<Parser>,
}

#[pymethods]
impl PyRegParser {
    #[new]
    fn new(path_or_file_like: PyObject) -> PyResult<Self> {
        let file_or_file_like = FileOrFileLike::from_pyobject(path_or_file_like)?;
        let boxed_read_seek = match file_or_file_like {
            FileOrFileLike::File(s) => {
                let file = File::open(s)?;

                let reader = BufReader::with_capacity(4096, file);

                Box::new(reader) as Box<dyn ReadSeek + Send>
            }
            FileOrFileLike::FileLike(f) => Box::new(f) as Box<dyn ReadSeek + Send>,
        };

        let parser =
            Parser::from_read_seek(boxed_read_seek, None, None, false).map_err(PyRegError)?;
        Ok(PyRegParser {
            inner: Some(parser),
        })
    }

    /// reg_keys(self, /)
    /// --
    ///
    /// Returns an iterator that yields reg keys as python objects.
    fn reg_keys(&mut self) -> PyResult<Py<PyRegKeysIterator>> {
        self.reg_keys_iterator()
    }

    fn open(&mut self, path: &str) -> PyResult<Option<Py<PyRegKey>>> {
        match &mut self.inner {
            Some(parser) => match parser.get_key(path, false) {
                Ok(key) => {
                    if let Some(key) = key {
                        let gil = Python::acquire_gil();
                        let py = gil.python();
                        let ret = PyRegKey::from_cell_key_node(py, key);
                        if let Ok(py_reg_key) = ret {
                            return Ok(Some(py_reg_key));
                        }
                    }
                }
                Err(e) => return Err(PyErr::new::<PyRuntimeError, _>(e.to_string())),
            },
            _ => return Ok(None),
        }
        Ok(None)
    }

    /// root(self, /)
    /// --
    ///
    /// Returns the root PyRegKey
    fn root(&mut self) -> PyResult<Option<Py<PyRegKey>>> {
        match &mut self.inner {
            Some(parser) => match parser.get_root_key() {
                Ok(key) => {
                    if let Some(key) = key {
                        let gil = Python::acquire_gil();
                        let py = gil.python();
                        let ret = PyRegKey::from_cell_key_node(py, key);
                        if let Ok(py_reg_key) = ret {
                            return Ok(Some(py_reg_key));
                        }
                    }
                }
                Err(e) => return Err(PyErr::new::<PyRuntimeError, _>(e.to_string())),
            },
            _ => return Ok(None),
        }
        Ok(None)
    }

    /// parent(self, /)
    /// --
    ///
    /// Returns the parent PyRegKey for the `key` parameter
    fn get_parent(&mut self, key: &mut PyRegKey) -> PyResult<Option<Py<PyRegKey>>> {
        match &mut self.inner {
            Some(parser) => match parser.get_parent_key(&mut key.inner) {
                Ok(key) => {
                    if let Some(key) = key {
                        let gil = Python::acquire_gil();
                        let py = gil.python();
                        let ret = PyRegKey::from_cell_key_node(py, key);
                        if let Ok(py_reg_key) = ret {
                            return Ok(Some(py_reg_key));
                        }
                    }
                }
                Err(e) => return Err(PyErr::new::<PyRuntimeError, _>(e.to_string())),
            },
            _ => return Ok(None),
        }
        Ok(None)
    }
}

impl PyRegParser {
    fn reg_keys_iterator(&mut self) -> PyResult<Py<PyRegKeysIterator>> {
        let gil = Python::acquire_gil();
        let py = gil.python();
        let mut inner = match self.inner.take() {
            Some(inner) => inner,
            None => {
                return Err(PyErr::new::<PyRuntimeError, _>(
                    "PyRegParser can only be used once",
                ));
            }
        };
        inner.init_key_iter();

        Py::new(py, PyRegKeysIterator { inner })
    }
}

#[pyclass]
pub struct PyRegKeysIterator {
    inner: Parser,
}

impl PyRegKeysIterator {
    pub(crate) fn reg_key_to_pyobject(
        reg_key_result: Result<CellKeyNode, PyRegError>,
        py: Python,
    ) -> PyObject {
        match reg_key_result {
            Ok(reg_key) => {
                match PyRegKey::from_cell_key_node(py, reg_key).map(|entry| entry.to_object(py)) {
                    Ok(py_reg_key) => py_reg_key,
                    Err(e) => e.to_object(py),
                }
            }
            Err(e) => {
                let err = PyErr::from(e);
                err.to_object(py)
            }
        }
    }

    fn next(&mut self) -> Option<PyObject> {
        let gil = Python::acquire_gil();
        let py = gil.python();
        self.inner
            .next_key_preorder(false)
            .map(|key| Self::reg_key_to_pyobject(Ok(key), py))
    }
}

#[pyproto]
impl PyIterProtocol for PyRegParser {
    fn __iter__(mut slf: PyRefMut<Self>) -> PyResult<Py<PyRegKeysIterator>> {
        slf.reg_keys()
    }
    fn __next__(_slf: PyRefMut<Self>) -> PyResult<Option<PyObject>> {
        Err(PyErr::new::<PyNotImplementedError, _>("Using `next()` over `PyRegParser` is not supported. Try iterating over `PyRegParser(...).reg_keys()`"))
    }
}

#[pyproto]
impl PyIterProtocol for PyRegKeysIterator {
    fn __iter__(slf: PyRefMut<Self>) -> PyResult<Py<PyRegKeysIterator>> {
        Ok(slf.into())
    }
    fn __next__(mut slf: PyRefMut<Self>) -> PyResult<Option<PyObject>> {
        Ok(slf.next())
    }
}

/// Parses a windows registry file.
#[pymodule]
fn notatin(py: Python, m: &PyModule) -> PyResult<()> {
    init_logging(py).ok();

    m.add_class::<PyRegParser>()?;
    m.add_class::<PyRegKey>()?;
    m.add_class::<PyRegValue>()?;

    Ok(())
}