//! SSR data-tools → clickable chips (definitions panel).

use serde_json::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlButtonElement};

pub fn wire_tool_chips(document: &Document) -> Result<(), JsValue> {
    let Some(root) = document.get_element_by_id("tool-meta") else {
        return Ok(());
    };
    let chips = document
        .get_element_by_id("tool-chips")
        .ok_or_else(|| JsValue::from_str("no tool-chips"))?;
    let def = document
        .get_element_by_id("tool-def")
        .ok_or_else(|| JsValue::from_str("no tool-def"))?;
    let raw = root.get_attribute("data-tools").unwrap_or_else(|| "[]".into());
    let tools: Value = serde_json::from_str(&raw).unwrap_or(Value::Array(vec![]));
    let arr = tools.as_array().cloned().unwrap_or_default();
    chips.set_inner_html("");
    for t in arr {
        let name = t
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        let btn = document
            .create_element("button")?
            .dyn_into::<HtmlButtonElement>()?;
        btn.set_type("button");
        btn.set_class_name("tool-chip");
        btn.set_text_content(Some(&name));
        let def_el = def.clone();
        let payload = t.to_string();
        let closure = Closure::wrap(Box::new(move |_e: web_sys::MouseEvent| {
            if let Ok(v) = serde_json::from_str::<Value>(&payload) {
                def_el.set_text_content(Some(
                    &serde_json::to_string_pretty(&v).unwrap_or(payload.clone()),
                ));
            } else {
                def_el.set_text_content(Some(&payload));
            }
            let _ = def_el.remove_attribute("hidden");
        }) as Box<dyn FnMut(_)>);
        btn.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
        closure.forget();
        chips.append_child(&btn)?;
    }
    Ok(())
}
