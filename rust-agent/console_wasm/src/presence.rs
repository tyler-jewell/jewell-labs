//! Poll /api/presence and paint idle/busy dots on the agent sidebar.

use serde_json::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, Response};

pub fn wire_presence(document: &Document) -> Result<(), JsValue> {
    // Initial paint + light poll for multi-agent ops view.
    let doc = document.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let _ = refresh_presence(&doc).await;
    });
    let doc2 = document.clone();
    let cb = Closure::wrap(Box::new(move || {
        let d = doc2.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = refresh_presence(&d).await;
        });
    }) as Box<dyn FnMut()>);
    let _ = web_sys::window()
        .unwrap()
        .set_interval_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(),
            2500,
        );
    cb.forget();
    Ok(())
}

async fn refresh_presence(document: &Document) -> Result<(), JsValue> {
    let resp = JsFuture::from(web_sys::window().unwrap().fetch_with_str("/api/presence")).await?;
    let resp: Response = resp.dyn_into()?;
    if !resp.ok() {
        return Ok(());
    }
    let text = JsFuture::from(resp.text()?).await?;
    let text = text.as_string().unwrap_or_default();
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let agents = v
        .get("agents")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();
    for a in agents {
        let id = a.get("agent_id").and_then(|x| x.as_str()).unwrap_or("");
        let status = a
            .get("status")
            .and_then(|x| x.as_str())
            .unwrap_or("idle");
        if id.is_empty() {
            continue;
        }
        paint_agent(document, id, status)?;
    }
    Ok(())
}

fn paint_agent(document: &Document, agent_id: &str, status: &str) -> Result<(), JsValue> {
    let selector = format!(r#"[data-agent="{agent_id}"]"#);
    let Ok(Some(el)) = document.query_selector(&selector) else {
        return Ok(());
    };
    el.set_attribute("data-presence", status)?;
    if let Some(dot) = el.query_selector(".presence")? {
        let el: Element = dot;
        el.set_class_name(&format!("presence presence-{status}"));
        el.set_attribute("title", status)?;
        el.set_attribute("aria-label", status)?;
    }
    // Name is already on the row; presence is the dot only (no duplicated meta line).
    Ok(())
}
