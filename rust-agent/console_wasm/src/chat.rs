//! Chat form + POST SSE stream using console_core parsers.

use console_core::{
    parse_sse_data_line, render_markdown, run_event_from_sse, RunEvent, RunKind, SseEvent,
};
use js_sys::{Function, Reflect, Uint8Array};
use serde_json::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Document, Element, HtmlButtonElement, HtmlFormElement, HtmlTextAreaElement, Request,
    RequestInit, RequestMode, Response, SubmitEvent,
};

pub fn wire_chat(document: &Document) -> Result<(), JsValue> {
    let Some(form_el) = document.get_element_by_id("chat-form") else {
        return Ok(());
    };
    let form: HtmlFormElement = form_el.dyn_into()?;
    let agent = form.get_attribute("data-agent").unwrap_or_default();
    let transcript = document
        .get_element_by_id("transcript")
        .ok_or_else(|| JsValue::from_str("no transcript"))?;
    let input: HtmlTextAreaElement = document
        .get_element_by_id("chat-input")
        .ok_or_else(|| JsValue::from_str("no input"))?
        .dyn_into()?;
    let btn_send: HtmlButtonElement = document
        .get_element_by_id("btn-send")
        .ok_or_else(|| JsValue::from_str("no send"))?
        .dyn_into()?;

    let agent_c = agent.clone();
    let transcript_c = transcript.clone();
    let input_c = input.clone();
    let btn_c = btn_send.clone();
    let document_c = document.clone();
    let closure = Closure::wrap(Box::new(move |e: SubmitEvent| {
        e.prevent_default();
        let agent = agent_c.clone();
        let transcript = transcript_c.clone();
        let input = input_c.clone();
        let btn = btn_c.clone();
        let document = document_c.clone();
        let message = input.value().trim().to_string();
        if message.is_empty() {
            return;
        }
        input.set_value("");
        btn.set_disabled(true);
        wasm_bindgen_futures::spawn_local(async move {
            let _ = run_chat_turn(&document, &transcript, &agent, &message).await;
            btn.set_disabled(false);
            let _ = input.focus();
        });
    }) as Box<dyn FnMut(_)>);
    form.add_event_listener_with_callback("submit", closure.as_ref().unchecked_ref())?;
    closure.forget();
    Ok(())
}

async fn run_chat_turn(
    document: &Document,
    transcript: &Element,
    agent: &str,
    message: &str,
) -> Result<(), JsValue> {
    clear_run_events(document)?;
    append_msg(transcript, "user", message, false)?;
    let assistant = append_msg(transcript, "assistant", "", true)?;
    let body = serde_json::json!({
        "agent_stem": agent,
        "message": message,
        "history": []
    });
    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::Cors);
    opts.set_body(&JsValue::from_str(&body.to_string()));
    let req = Request::new_with_str_and_init("/api/chat/stream", &opts)?;
    req.headers().set("content-type", "application/json")?;
    let resp_val = JsFuture::from(web_sys::window().unwrap().fetch_with_request(&req)).await?;
    let resp: Response = resp_val.dyn_into()?;
    if !resp.ok() {
        set_md(&assistant, &format!("Error: HTTP {}", resp.status()), false)?;
        push_run(
            document,
            &RunEvent {
                kind: RunKind::Error,
                label: "error".into(),
                detail: format!("HTTP {}", resp.status()),
            },
        )?;
        return Ok(());
    }
    let body_stream = resp
        .body()
        .ok_or_else(|| JsValue::from_str("no body stream"))?;
    let get_reader = Reflect::get(&body_stream, &JsValue::from_str("getReader"))?;
    let get_reader: Function = get_reader.dyn_into()?;
    let reader = get_reader.call0(&body_stream)?;
    let mut full = String::new();
    let mut line_buf = String::new();
    let mut saw_tool = false;
    let mut last_model_push = 0usize;
    loop {
        let read = Reflect::get(&reader, &JsValue::from_str("read"))?;
        let read: Function = read.dyn_into()?;
        let result = JsFuture::from(js_sys::Promise::from(read.call0(&reader)?)).await?;
        let done = Reflect::get(&result, &JsValue::from_str("done"))?
            .as_bool()
            .unwrap_or(true);
        if done {
            break;
        }
        let value = Reflect::get(&result, &JsValue::from_str("value"))?;
        let arr = Uint8Array::new(&value);
        let mut bytes = vec![0u8; arr.length() as usize];
        arr.copy_to(&mut bytes);
        line_buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(pos) = line_buf.find('\n') {
            let line = line_buf[..pos].to_string();
            line_buf = line_buf[pos + 1..].to_string();
            let ev = parse_sse_data_line(&line);
            if let Some(re) = run_event_from_sse(&ev) {
                // Coalesce model chips: only push every ~40 chars growth.
                if re.kind == RunKind::Model {
                    if full.len().saturating_sub(last_model_push) >= 40 || last_model_push == 0 {
                        push_run(document, &re)?;
                        last_model_push = full.len() + re.detail.len();
                    }
                } else {
                    push_run(document, &re)?;
                }
            }
            match ev {
                SseEvent::Skip | SseEvent::Done => {}
                SseEvent::Error(e) => set_md(&assistant, &format!("Error: {e}"), false)?,
                SseEvent::Delta(d) => {
                    full.push_str(&d);
                    set_md(&assistant, &full, true)?;
                }
                SseEvent::ToolCall(tc) => {
                    saw_tool = true;
                    full.clear();
                    last_model_push = 0;
                    set_md(&assistant, "", true)?;
                    let name = tc.get("name").and_then(|n| n.as_str()).unwrap_or("tool");
                    append_tool(transcript, "tool_call", name, &tc)?;
                }
                SseEvent::ToolResult(tr) => {
                    saw_tool = true;
                    full.clear();
                    last_model_push = 0;
                    set_md(&assistant, "", true)?;
                    let name = tr.get("name").and_then(|n| n.as_str()).unwrap_or("tool");
                    let ok = tr.get("ok").and_then(|o| o.as_bool()).unwrap_or(false);
                    let title = format!("{name} {}", if ok { "ok" } else { "err" });
                    append_tool(transcript, "tool_result", &title, &tr)?;
                }
                SseEvent::Other(_) => {}
            }
        }
    }
    if full.is_empty() && !saw_tool {
        set_md(&assistant, "(empty response)", false)?;
    } else if !full.is_empty() {
        set_md(&assistant, &full, false)?;
    } else {
        let _ = assistant.remove();
    }
    Ok(())
}

fn clear_run_events(document: &Document) -> Result<(), JsValue> {
    if let Some(ol) = document.get_element_by_id("run-events") {
        ol.set_inner_html("");
    }
    if let Some(wrap) = document.get_element_by_id("run-events-wrap") {
        let _ = wrap.set_attribute("hidden", "");
    }
    Ok(())
}

fn push_run(document: &Document, ev: &RunEvent) -> Result<(), JsValue> {
    let Some(ol) = document.get_element_by_id("run-events") else {
        return Ok(());
    };
    if let Some(wrap) = document.get_element_by_id("run-events-wrap") {
        let _ = wrap.remove_attribute("hidden");
    }
    let li = document.create_element("li")?;
    li.set_class_name(&format!("run-ev run-ev-{}", ev.kind.as_str()));
    let kind = document.create_element("span")?;
    kind.set_class_name("run-ev-kind");
    kind.set_text_content(Some(ev.kind.as_str()));
    let label = document.create_element("span")?;
    label.set_class_name("run-ev-label");
    label.set_text_content(Some(&ev.label));
    let detail = document.create_element("span")?;
    detail.set_class_name("run-ev-detail");
    detail.set_text_content(Some(&ev.detail));
    li.append_child(&kind)?;
    li.append_child(&label)?;
    li.append_child(&detail)?;
    ol.append_child(&li)?;
    Ok(())
}

fn append_msg(
    transcript: &Element,
    role: &str,
    text: &str,
    streaming: bool,
) -> Result<Element, JsValue> {
    let document = web_sys::window().unwrap().document().unwrap();
    let div = document.create_element("div")?;
    let mut class = format!("msg {role}");
    if streaming {
        class.push_str(" streaming");
    }
    div.set_class_name(&class);
    let role_el = document.create_element("span")?;
    role_el.set_class_name("role");
    role_el.set_text_content(Some(role));
    let body = document.create_element("div")?;
    body.set_class_name("content md-body");
    set_md(&body, text, streaming || role == "user")?;
    div.append_child(&role_el)?;
    div.append_child(&body)?;
    transcript.append_child(&div)?;
    scroll_bottom(transcript);
    Ok(body)
}

fn append_tool(
    transcript: &Element,
    kind: &str,
    title: &str,
    payload: &Value,
) -> Result<(), JsValue> {
    let document = web_sys::window().unwrap().document().unwrap();
    let div = document.create_element("div")?;
    div.set_class_name(&format!("msg tool {kind}"));
    let role_el = document.create_element("span")?;
    role_el.set_class_name("role");
    role_el.set_text_content(Some(if kind == "tool_call" {
        "tool →"
    } else {
        "tool ←"
    }));
    let body = document.create_element("div")?;
    body.set_class_name("content");
    let head = document.create_element("div")?;
    head.set_class_name("tool-title");
    head.set_text_content(Some(title));
    let pre = document.create_element("pre")?;
    pre.set_class_name("tool-payload");
    pre.set_text_content(Some(
        &serde_json::to_string_pretty(payload).unwrap_or_else(|_| payload.to_string()),
    ));
    body.append_child(&head)?;
    body.append_child(&pre)?;
    div.append_child(&role_el)?;
    div.append_child(&body)?;
    transcript.append_child(&div)?;
    scroll_bottom(transcript);
    Ok(())
}

fn set_md(el: &Element, text: &str, plain: bool) -> Result<(), JsValue> {
    if plain || text.is_empty() {
        el.set_class_name("content md-body md-plain");
        el.set_text_content(Some(text));
    } else {
        el.set_class_name("content md-body");
        el.set_inner_html(&render_markdown(text));
    }
    Ok(())
}

fn scroll_bottom(transcript: &Element) {
    if let Ok(h) = Reflect::get(transcript, &JsValue::from_str("scrollHeight")) {
        let _ = Reflect::set(transcript, &JsValue::from_str("scrollTop"), &h);
    }
}
