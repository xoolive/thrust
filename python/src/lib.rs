//! Python bindings for thrust core functionalities.

use pyo3::prelude::*;

pub mod airac;
pub mod airports;
pub mod airspaces;
pub mod airways;
pub mod field15;
pub mod intervals;
pub mod navpoints;

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

    let field15_mod = field15::init(py)?;
    m.add_submodule(&field15_mod)?;
    modules.set_item("thrust.core.field15", &field15_mod)?;

    let airports_mod = airports::init(py)?;
    m.add_submodule(&airports_mod)?;
    modules.set_item("thrust.core.airports", &airports_mod)?;

    let airac_mod = airac::init(py)?;
    m.add_submodule(&airac_mod)?;
    modules.set_item("thrust.core.airac", &airac_mod)?;

    let airways_mod = airways::init(py)?;
    m.add_submodule(&airways_mod)?;
    modules.set_item("thrust.core.airways", &airways_mod)?;

    let airspaces_mod = airspaces::init(py)?;
    m.add_submodule(&airspaces_mod)?;
    modules.set_item("thrust.core.airspaces", &airspaces_mod)?;

    let navpoints_mod = navpoints::init(py)?;
    m.add_submodule(&navpoints_mod)?;
    modules.set_item("thrust.core.navpoints", &navpoints_mod)?;

    Ok(())
}
