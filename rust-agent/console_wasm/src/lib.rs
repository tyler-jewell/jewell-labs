//! Browser chat island: stream + tool cards + markdown via console_core.
//! Only wasm-bindgen-generated JS is required to load this module.

mod chat;
mod chips;
mod sessions;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    if let Err(e) = boot() {
        web_sys::console::error_1(&e);
    }
}

fn boot() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window.document().ok_or_else(|| JsValue::from_str("no document"))?;
    chips::wire_tool_chips(&document)?;
    chat::wire_chat(&document)?;
    sessions::wire_sessions(&document)?;
    Ok(())
}
