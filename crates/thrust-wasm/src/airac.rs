use serde::Serialize;
use wasm_bindgen::prelude::*;

use thrust::data::airac::{
    airac_code_from_date as rs_airac_code_from_date, airac_interval as rs_airac_interval,
    effective_date_from_airac_code as rs_effective_date_from_airac_code,
};

#[derive(Clone, Debug, Serialize)]
struct AiracIntervalRecord {
    begin: String,
    end: String,
}

#[wasm_bindgen]
pub fn airac_code_from_date(date: String) -> Result<String, JsValue> {
    let parsed = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|e| JsValue::from_str(&format!("invalid date '{date}': {e}")))?;
    Ok(rs_airac_code_from_date(parsed))
}

#[wasm_bindgen]
pub fn effective_date_from_airac_code(airac_code: String) -> Result<String, JsValue> {
    let date = rs_effective_date_from_airac_code(&airac_code).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(date.format("%Y-%m-%d").to_string())
}

#[wasm_bindgen]
pub fn airac_interval(airac_code: String) -> Result<JsValue, JsValue> {
    let (begin, end) = rs_airac_interval(&airac_code).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let payload = AiracIntervalRecord {
        begin: begin.format("%Y-%m-%d").to_string(),
        end: end.format("%Y-%m-%d").to_string(),
    };
    serde_wasm_bindgen::to_value(&payload).map_err(|e| JsValue::from_str(&e.to_string()))
}
