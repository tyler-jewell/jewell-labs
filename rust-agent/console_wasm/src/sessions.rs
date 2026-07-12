//! Session list for agent (uses host /api/sessions).

use serde_json::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, Request, RequestInit, RequestMode, Response};

pub fn wire_sessions(document: &Document) -> Result<(), JsValue> {
    let Some(list) = document.get_element_by_id("session-list") else {
        return Ok(());
    };
    let agent = list
        .closest(".sessions")?
        .and_then(|el| el.get_attribute("data-agent"))
        .unwrap_or_default();
    if agent.is_empty() {
        return Ok(());
    }
    wasm_bindgen_futures::spawn_local({
        let list = list.clone();
        let agent = agent.clone();
        async move {
            let _ = load_sessions(&list, &agent).await;
        }
    });
    if let Some(btn) = document.get_element_by_id("btn-new-session") {
        let agent2 = agent.clone();
        let list2 = list.clone();
        let closure = Closure::wrap(Box::new(move |_e: web_sys::MouseEvent| {
            let agent = agent2.clone();
            let list = list2.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let _ = create_session(&agent).await;
                let _ = load_sessions(&list, &agent).await;
            });
        }) as Box<dyn FnMut(_)>);
        btn.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }
    Ok(())
}

async fn load_sessions(list: &Element, agent: &str) -> Result<(), JsValue> {
    list.set_inner_html("<li class=\"muted\">Loading…</li>");
    let url = format!("/api/sessions?agent={}", js_sys::encode_uri_component(agent));
    let resp = JsFuture::from(web_sys::window().unwrap().fetch_with_str(&url)).await?;
    let resp: Response = resp.dyn_into()?;
    if !resp.ok() {
        list.set_inner_html("<li class=\"muted\">Failed to load sessions</li>");
        return Ok(());
    }
    let text = JsFuture::from(resp.text()?).await?;
    let text = text.as_string().unwrap_or_default();
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let sessions = v
        .get("sessions")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    list.set_inner_html("");
    let document = web_sys::window().unwrap().document().unwrap();
    if sessions.is_empty() {
        list.set_inner_html("<li class=\"muted\">No sessions yet.</li>");
        return Ok(());
    }
    for s in sessions {
        let id = s.get("id").and_then(|x| x.as_str()).unwrap_or("?");
        let li = document.create_element("li")?;
        li.set_text_content(Some(id));
        list.append_child(&li)?;
    }
    Ok(())
}

async fn create_session(agent: &str) -> Result<(), JsValue> {
    let id = uuidish();
    let body = serde_json::json!({
        "agent_stem": agent,
        "session_id": id,
        "messages": []
    });
    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::Cors);
    opts.set_body(&JsValue::from_str(&body.to_string()));
    let req = Request::new_with_str_and_init("/api/sessions", &opts)?;
    req.headers().set("content-type", "application/json")?;
    let _ = JsFuture::from(web_sys::window().unwrap().fetch_with_request(&req)).await?;
    Ok(())
}

fn uuidish() -> String {
    let arr = js_sys::Uint8Array::new_with_length(16);
    for i in 0..16 {
        arr.set_index(i, (js_sys::Math::random() * 256.0) as u8);
    }
    let mut s = String::new();
    for i in 0..16 {
        s.push_str(&format!("{:02x}", arr.get_index(i)));
        if i == 3 || i == 5 || i == 7 || i == 9 {
            s.push('-');
        }
    }
    s
}
