//! Python bindings for thrust core functionalities.

use pyo3::prelude::*;

pub mod intervals;
#[cfg(feature = "polars")]
pub mod kalman;

#[pymodule]
#[pyo3(name = "core")]
fn thrust(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let interval_mod = intervals::init(py)?;
    m.add_submodule(&interval_mod)?;

    // This works "from thrust.core import intervals" in Python
    // The following allows to import as "import thrust.core.intervals"
    // or "from thrust.core.intervals import ..."
    let sys = py.import("sys")?;
    let modules = sys.getattr("modules")?;
    modules.set_item("thrust.core.intervals", &interval_mod)?;

    #[cfg(feature = "polars")]
    {
        let kalman_mod = kalman::init(py)?;
        m.add_submodule(&kalman_mod)?;
    }

    Ok(())
}
