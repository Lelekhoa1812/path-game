use wasm_bindgen::prelude::*;

pub mod generator;
pub mod grid;
pub mod rng;
pub mod solver;
mod types;

pub use types::{GenerateMetrics, GenerateRequest, PuzzleResponse, Waypoint};

#[wasm_bindgen]
pub fn generate_puzzle(request: JsValue) -> Result<JsValue, JsValue> {
    let request: GenerateRequest = serde_wasm_bindgen::from_value(request)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let response = generate_puzzle_for_test(request);
    serde_wasm_bindgen::to_value(&response).map_err(|err| JsValue::from_str(&err.to_string()))
}

pub fn generate_puzzle_for_test(request: GenerateRequest) -> PuzzleResponse {
    generator::generate(request)
}
