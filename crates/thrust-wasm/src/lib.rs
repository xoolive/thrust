mod airac;
mod eurocontrol;
mod faa_arcgis;
mod field15;
mod models;
mod nasr;
mod utils;

use wasm_bindgen::prelude::*;

pub use airac::{airac_code_from_date, airac_interval, effective_date_from_airac_code};
pub use eurocontrol::EurocontrolResolver;
pub use faa_arcgis::FaaArcgisResolver;
pub use field15::parse_field15;
pub use nasr::NasrResolver;
use utils::set_panic_hook;

#[wasm_bindgen]
pub fn run() -> Result<(), JsValue> {
    set_panic_hook();
    Ok(())
}

#[wasm_bindgen]
pub fn wasm_build_profile() -> String {
    if cfg!(debug_assertions) {
        "debug".to_string()
    } else {
        "release".to_string()
    }
}
