use thrust::intervals::{Interval, IntervalCollection};

use numpy::{PyArray1, PyReadonlyArray1};
use pyo3::types::IntoPyDict;
use pyo3::{prelude::*, types::PyDict};

fn get_ic(start: PyReadonlyArray1<i64>, stop: PyReadonlyArray1<i64>) -> IntervalCollection<i64> {
    let size1 = start.len().unwrap();
    let size2 = stop.len().unwrap();
    let size = std::cmp::min(size1, size2);

    let mut elts = Vec::<Interval<i64>>::with_capacity(size);
    for i in 0..size {
        elts.push(Interval {
            start: *start.get(i).unwrap(),
            stop: *stop.get(i).unwrap(),
        })
    }
    IntervalCollection { elts }
}

#[pyfunction]
fn interval_and(py: Python, start1: i64, stop1: i64, start2: i64, stop2: i64) -> PyResult<Bound<PyDict>> {
    let left = Interval {
        start: start1,
        stop: stop1,
    };
    let right = Interval {
        start: start2,
        stop: stop2,
    };
    let res = match &left & &right {
        None => [("empty", true)].into_py_dict(py),
        Some(Interval { start, stop }) => [("start", start), ("stop", stop)].into_py_dict(py),
    };
    res
}

#[pyfunction]
fn collection_and<'a>(
    py: Python<'a>,
    start1: PyReadonlyArray1<i64>,
    stop1: PyReadonlyArray1<i64>,
    start2: PyReadonlyArray1<i64>,
    stop2: PyReadonlyArray1<i64>,
) -> PyResult<Bound<'a, PyDict>> {
    let left = get_ic(start1, stop1);
    let right = get_ic(start2, stop2);
    let res = &left & &right;
    let start: Vec<i64> = res.elts.iter().map(|elt| elt.start).collect();
    let stop: Vec<i64> = res.elts.iter().map(|elt| elt.stop).collect();

    let wrapped_res = PyDict::new(py);
    wrapped_res.set_item("start", PyArray1::from_vec(py, start))?;
    wrapped_res.set_item("stop", PyArray1::from_vec(py, stop))?;
    Ok(wrapped_res)
}

#[pyfunction]
fn collection_andi<'a>(
    py: Python<'a>,
    start1: PyReadonlyArray1<i64>,
    stop1: PyReadonlyArray1<i64>,
    start2: i64,
    stop2: i64,
) -> PyResult<Bound<'a, PyDict>> {
    let left = get_ic(start1, stop1);
    let right = Interval {
        start: start2,
        stop: stop2,
    };

    let res = &left & &right;
    let start: Vec<i64> = res.elts.iter().map(|elt| elt.start).collect();
    let stop: Vec<i64> = res.elts.iter().map(|elt| elt.stop).collect();

    let wrapped_res = PyDict::new(py);
    wrapped_res.set_item("start", PyArray1::from_vec(py, start))?;
    wrapped_res.set_item("stop", PyArray1::from_vec(py, stop))?;
    Ok(wrapped_res)
}

#[pyfunction]
fn interval_add(py: Python, start1: i64, stop1: i64, start2: i64, stop2: i64) -> PyResult<Bound<PyDict>> {
    let left = Interval {
        start: start1,
        stop: stop1,
    };
    let right = Interval {
        start: start2,
        stop: stop2,
    };

    let res = left + right;
    let start: Vec<i64> = res.elts.iter().map(|elt| elt.start).collect();
    let stop: Vec<i64> = res.elts.iter().map(|elt| elt.stop).collect();

    let wrapped_res = PyDict::new(py);
    wrapped_res.set_item("start", PyArray1::from_vec(py, start))?;
    wrapped_res.set_item("stop", PyArray1::from_vec(py, stop))?;
    Ok(wrapped_res)
}

#[pyfunction]
fn collection_add<'a>(
    py: Python<'a>,
    start1: PyReadonlyArray1<i64>,
    stop1: PyReadonlyArray1<i64>,
    start2: PyReadonlyArray1<i64>,
    stop2: PyReadonlyArray1<i64>,
) -> PyResult<Bound<'a, PyDict>> {
    let left = get_ic(start1, stop1);
    let right = get_ic(start2, stop2);
    let res = left + right;

    let start: Vec<i64> = res.elts.iter().map(|elt| elt.start).collect();
    let stop: Vec<i64> = res.elts.iter().map(|elt| elt.stop).collect();

    let wrapped_res = PyDict::new(py);
    wrapped_res.set_item("start", PyArray1::from_vec(py, start))?;
    wrapped_res.set_item("stop", PyArray1::from_vec(py, stop))?;
    Ok(wrapped_res)
}

#[pyfunction]
fn collection_addi<'a>(
    py: Python<'a>,
    start1: PyReadonlyArray1<i64>,
    stop1: PyReadonlyArray1<i64>,
    start2: i64,
    stop2: i64,
) -> PyResult<Bound<'a, PyDict>> {
    let left = get_ic(start1, stop1);
    let right = Interval {
        start: start2,
        stop: stop2,
    };
    let res = &left + &right;

    let start: Vec<i64> = res.elts.iter().map(|elt| elt.start).collect();
    let stop: Vec<i64> = res.elts.iter().map(|elt| elt.stop).collect();

    let wrapped_res = PyDict::new(py);
    wrapped_res.set_item("start", PyArray1::from_vec(py, start))?;
    wrapped_res.set_item("stop", PyArray1::from_vec(py, stop))?;
    Ok(wrapped_res)
}

#[pyfunction]
fn interval_sub(py: Python, start1: i64, stop1: i64, start2: i64, stop2: i64) -> PyResult<Bound<PyDict>> {
    let left = Interval {
        start: start1,
        stop: stop1,
    };
    let right = Interval {
        start: start2,
        stop: stop2,
    };

    let res = left - right;
    let start: Vec<i64> = res.elts.iter().map(|elt| elt.start).collect();
    let stop: Vec<i64> = res.elts.iter().map(|elt| elt.stop).collect();

    let wrapped_res = PyDict::new(py);
    wrapped_res.set_item("start", PyArray1::from_vec(py, start))?;
    wrapped_res.set_item("stop", PyArray1::from_vec(py, stop))?;
    Ok(wrapped_res)
}

#[pyfunction]
fn collection_sub<'a>(
    py: Python<'a>,
    start1: PyReadonlyArray1<i64>,
    stop1: PyReadonlyArray1<i64>,
    start2: PyReadonlyArray1<i64>,
    stop2: PyReadonlyArray1<i64>,
) -> PyResult<Bound<'a, PyDict>> {
    let left = get_ic(start1, stop1);
    let right = get_ic(start2, stop2);
    let res = left - right;

    let start: Vec<i64> = res.elts.iter().map(|elt| elt.start).collect();
    let stop: Vec<i64> = res.elts.iter().map(|elt| elt.stop).collect();

    let wrapped_res = PyDict::new(py);
    wrapped_res.set_item("start", PyArray1::from_vec(py, start))?;
    wrapped_res.set_item("stop", PyArray1::from_vec(py, stop))?;
    Ok(wrapped_res)
}

#[pyfunction]
fn collection_subi<'a>(
    py: Python<'a>,
    start1: PyReadonlyArray1<i64>,
    stop1: PyReadonlyArray1<i64>,
    start2: i64,
    stop2: i64,
) -> PyResult<Bound<'a, PyDict>> {
    let left = get_ic(start1, stop1);
    let right = Interval {
        start: start2,
        stop: stop2,
    };
    let res = left - right;

    let start: Vec<i64> = res.elts.iter().map(|elt| elt.start).collect();
    let stop: Vec<i64> = res.elts.iter().map(|elt| elt.stop).collect();

    let wrapped_res = PyDict::new(py);
    wrapped_res.set_item("start", PyArray1::from_vec(py, start))?;
    wrapped_res.set_item("stop", PyArray1::from_vec(py, stop))?;
    Ok(wrapped_res)
}

pub fn init(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, "intervals")?;

    m.add_function(wrap_pyfunction!(interval_and, &m)?)?;
    m.add_function(wrap_pyfunction!(collection_and, &m)?)?;
    m.add_function(wrap_pyfunction!(collection_andi, &m)?)?;

    m.add_function(wrap_pyfunction!(interval_add, &m)?)?;
    m.add_function(wrap_pyfunction!(collection_add, &m)?)?;
    m.add_function(wrap_pyfunction!(collection_addi, &m)?)?;

    m.add_function(wrap_pyfunction!(interval_sub, &m)?)?;
    m.add_function(wrap_pyfunction!(collection_sub, &m)?)?;
    m.add_function(wrap_pyfunction!(collection_subi, &m)?)?;

    Ok(m)
}
