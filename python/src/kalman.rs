/*
use ::thrust::kalman::kalman6d;
use numpy::{PyArray2, PyArray3};
use polars::prelude::PyDataFrame;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

#[pyfunction]
fn kalman6d_rs(py: Python, pydf: PyDataFrame) -> PyResult<Bound<PyDict>> {
    // Convert PyDataFrame to native DataFrame
    let df = pydf.into();
    // Pass df to your Rust logic (e.g., kalman6d)
    kalman6d(df)
        .map(|(x_pre, x_cor, p_pre, p_cor)| {
            let wrapped_res = PyDict::new(py);
            wrapped_res
                .set_item("x_pre", PyArray2::from_owned_array(py, x_pre))
                .unwrap();
            wrapped_res
                .set_item("x_cor", PyArray2::from_owned_array(py, x_cor))
                .unwrap();
            wrapped_res
                .set_item("p_pre", PyArray3::from_owned_array(py, p_pre))
                .unwrap();
            wrapped_res
                .set_item("p_cor", PyArray3::from_owned_array(py, p_cor))
                .unwrap();
            wrapped_res
        })
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

pub fn init(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, "kalman")?;

    m.add_function(wrap_pyfunction!(kalman6d_rs, &m)?)?;

    Ok(m)
}
*/
