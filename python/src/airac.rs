use chrono::NaiveDate;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use thrust::data::airac::{
    airac_code_from_date as rs_airac_code_from_date, airac_interval as rs_airac_interval,
    effective_date_from_airac_code as rs_effective_date_from_airac_code,
};

fn parse_date(date: &str) -> Result<NaiveDate, PyErr> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| PyValueError::new_err(format!("invalid date '{date}': {e}")))
}

#[pyfunction]
fn airac_code_from_date(date: String) -> PyResult<String> {
    let parsed = parse_date(&date)?;
    Ok(rs_airac_code_from_date(parsed))
}

#[pyfunction]
fn effective_date_from_airac_code(airac_code: String) -> PyResult<String> {
    let date = rs_effective_date_from_airac_code(&airac_code).map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(date.format("%Y-%m-%d").to_string())
}

#[pyfunction]
fn airac_interval(airac_code: String) -> PyResult<(String, String)> {
    let (begin, end) = rs_airac_interval(&airac_code).map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok((begin.format("%Y-%m-%d").to_string(), end.format("%Y-%m-%d").to_string()))
}

pub fn init(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, "airac")?;
    m.add_function(wrap_pyfunction!(airac_code_from_date, &m)?)?;
    m.add_function(wrap_pyfunction!(effective_date_from_airac_code, &m)?)?;
    m.add_function(wrap_pyfunction!(airac_interval, &m)?)?;
    Ok(m)
}
