//! Evals: pass/fail list + structured result card (+ multi-harness compare view).

use serde_json::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Document, Element, HtmlButtonElement, HtmlSelectElement, Request, RequestInit, RequestMode,
    Response,
};

pub fn wire_evals(document: &Document) -> Result<(), JsValue> {
    let Some(list) = document.get_element_by_id("eval-run-list") else {
        return Ok(());
    };
    let d0 = document.clone();
    let l0 = list.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let _ = reload(&d0, &l0, None).await;
    });
    bind_run(document, &list, "btn-run-team", "team")?;
    bind_run(document, &list, "btn-run-agent", "agent")?;
    bind_run(document, &list, "btn-run-catalog", "catalog")?;
    bind_run(document, &list, "btn-run-compare", "compare")?;
    Ok(())
}

fn bind_run(
    document: &Document,
    list: &Element,
    btn_id: &str,
    suite: &'static str,
) -> Result<(), JsValue> {
    let Some(btn) = document.get_element_by_id(btn_id) else {
        return Ok(());
    };
    let d = document.clone();
    let list = list.clone();
    let c = Closure::wrap(Box::new(move |_e: web_sys::MouseEvent| {
        let d = d.clone();
        let list = list.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let agent = if suite == "agent" {
                Some(sel(&d, "eval-agent").unwrap_or_else(|| "core/orchestrator".into()))
            } else {
                None
            };
            let _ = run(&d, &list, suite, agent).await;
        });
    }) as Box<dyn FnMut(_)>);
    btn.add_event_listener_with_callback("click", c.as_ref().unchecked_ref())?;
    c.forget();
    Ok(())
}

async fn reload(document: &Document, list: &Element, pick: Option<&str>) -> Result<(), JsValue> {
    list.set_inner_html("<li class=\"muted\">Loading…</li>");
    let v: Value =
        serde_json::from_str(&get("/api/evals/runs?limit=40").await?).unwrap_or(Value::Null);
    let runs = v
        .get("runs")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    if runs.is_empty() {
        list.set_inner_html("<li class=\"muted\">No runs yet</li>");
        return Ok(());
    }
    list.set_inner_html("");
    let doc = list.owner_document().unwrap();
    let mut first = None::<String>;
    for run in &runs {
        let id = run
            .get("id")
            .and_then(|x| x.as_str())
            .unwrap_or("?")
            .to_string();
        first.get_or_insert_with(|| id.clone());
        let label = run.get("label").and_then(|x| x.as_str()).unwrap_or("Run");
        let ok = run.get("passed").and_then(|x| x.as_bool()).unwrap_or(false);
        let kind = run.get("kind").and_then(|x| x.as_str()).unwrap_or("");
        let (b, t) = if kind == "catalog" {
            ("compare", "CAT")
        } else if kind == "compare" {
            ("compare", "CMP")
        } else if ok {
            ("pass", "PASS")
        } else {
            ("fail", "FAIL")
        };
        let li = doc.create_element("li")?;
        li.set_class_name("eval-run-item");
        li.set_inner_html(&format!(
            "<button type=\"button\" class=\"eval-run-btn\" data-id=\"{id}\">\
             <span class=\"eval-badge {b}\">{t}</span><span class=\"eval-run-label\">{label}</span>\
             <span class=\"eval-run-score\">{sc}</span><span class=\"eval-run-when\">{wh}</span></button>",
            sc = score(run),
            wh = ago(run.get("created").and_then(|x| x.as_str())),
        ));
        let id2 = id.clone();
        let d = document.clone();
        let l = list.clone();
        if let Some(btn) = li.query_selector("button")? {
            let c = Closure::wrap(Box::new(move |_e: web_sys::MouseEvent| {
                let id2 = id2.clone();
                let d = d.clone();
                let l = l.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    hilite(&l, &id2);
                    let _ = show(&d, &id2).await;
                });
            }) as Box<dyn FnMut(_)>);
            btn.add_event_listener_with_callback("click", c.as_ref().unchecked_ref())?;
            c.forget();
        }
        list.append_child(&li)?;
    }
    if let Some(id) = pick.map(|s| s.to_string()).or(first) {
        hilite(list, &id);
        let _ = show(document, &id).await;
    }
    Ok(())
}

fn hilite(list: &Element, id: &str) {
    let kids = list.children();
    for i in 0..kids.length() {
        if let Some(el) = kids.item(i) {
            let _ = el.class_list().remove_1("selected");
            if el
                .query_selector("button")
                .ok()
                .flatten()
                .and_then(|b| b.get_attribute("data-id"))
                .as_deref()
                == Some(id)
            {
                let _ = el.class_list().add_1("selected");
            }
        }
    }
}

async fn show(document: &Document, id: &str) -> Result<(), JsValue> {
    let Some(card) = document.get_element_by_id("eval-result") else {
        return Ok(());
    };
    card.set_inner_html("<p class=\"muted\">Loading…</p>");
    let url = format!("/api/evals/runs/{}", js_sys::encode_uri_component(id));
    let v: Value = serde_json::from_str(&get(&url).await?).unwrap_or(Value::Null);
    paint(&card, &v)
}

fn paint(card: &Element, r: &Value) -> Result<(), JsValue> {
    let kind = r.get("kind").and_then(|x| x.as_str()).unwrap_or("");
    let is_catalog = kind == "catalog"
        || r.get("id")
            .and_then(|x| x.as_str())
            .map(|s| s.starts_with("catalog-"))
            .unwrap_or(false);
    let is_compare = kind == "compare"
        || r.get("id")
            .and_then(|x| x.as_str())
            .map(|s| s.starts_with("compare-"))
            .unwrap_or(false)
        || r.get("summary")
            .and_then(|s| s.get("harnesses"))
            .and_then(|h| h.as_object())
            .is_some();

    if is_catalog {
        return paint_catalog(card, r);
    }
    if is_compare {
        return paint_compare(card, r);
    }

    let s = r.get("summary");
    let total = s
        .and_then(|x| x.get("total"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0) as usize;
    let correct = s
        .and_then(|x| x.get("correct"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0) as usize;
    let acc = s
        .and_then(|x| x.get("accuracy"))
        .and_then(|x| x.as_f64())
        .unwrap_or(0.0);
    let ok = (total > 0 && correct == total) || (total == 0 && acc >= 1.0);
    let agent = r.get("agent").and_then(|x| x.as_str()).unwrap_or("");
    let stem = agent.rsplit_once('/').map(|(_, s)| s).unwrap_or(agent);
    let rid = r.get("id").and_then(|x| x.as_str()).unwrap_or("");
    let title = if rid.starts_with("agent-introspection") {
        "Team gate"
    } else if stem.is_empty() {
        "Run"
    } else {
        stem
    };
    let (b, t) = if ok {
        ("pass", "PASS")
    } else {
        ("fail", "FAIL")
    };
    let sc = if total > 0 {
        format!("{correct}/{total}")
    } else {
        format!("{:.0}%", acc * 100.0)
    };
    let tracks = r
        .get("tracks")
        .and_then(|t| t.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let meta = match (stem.is_empty(), tracks.is_empty()) {
        (_, true) if stem.is_empty() => String::new(),
        (true, false) => tracks,
        (false, true) => stem.into(),
        _ => format!("{stem} · {tracks}"),
    };
    let mut cases = String::from("<ul class=\"eval-cases\">");
    if let Some(arr) = r.get("cases").and_then(|c| c.as_array()) {
        for c in arr {
            let cid = c.get("id").and_then(|x| x.as_str()).unwrap_or("case");
            let cok = c.get("correct").and_then(|x| x.as_bool()).unwrap_or(false);
            let (cls, m) = if cok { ("ok", "✓") } else { ("bad", "✗") };
            cases.push_str(&format!(
                "<li class=\"eval-case {cls}\"><span class=\"eval-case-mark\">{m}</span> {cid}</li>"
            ));
        }
    }
    cases.push_str("</ul>");
    card.set_inner_html(&format!(
        "<div class=\"eval-result-head\"><span class=\"eval-badge {b} large\">{t}</span>\
         <div class=\"eval-result-main\"><div class=\"eval-result-title\">{title}</div>\
         <div class=\"eval-result-meta\">{meta}</div></div>\
         <div class=\"eval-result-side\"><div class=\"eval-result-score\">{sc}</div>\
         <div class=\"eval-result-when\">{wh}</div></div></div>{cases}",
        wh = ago(r.get("created").and_then(|x| x.as_str())),
    ));
    Ok(())
}

fn paint_compare(card: &Element, r: &Value) -> Result<(), JsValue> {
    let s = r.get("summary");
    let harnesses = s
        .and_then(|x| x.get("harnesses"))
        .and_then(|h| h.as_object());
    let rid = r.get("id").and_then(|x| x.as_str()).unwrap_or("compare");
    let total = s
        .and_then(|x| x.get("total"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let correct = s
        .and_then(|x| x.get("correct"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let mut board = String::from(
        "<table class=\"eval-board\"><thead><tr>\
         <th>Harness</th><th>avg</th><th>solid</th><th>pass</th><th>scored</th><th>skip</th><th>err</th>\
         </tr></thead><tbody>",
    );
    if let Some(hs) = harnesses {
        for (name, row) in hs {
            let avg = row.get("avg_score").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let solid = row
                .get("solid_base")
                .and_then(|x| x.as_f64())
                .unwrap_or(0.0);
            let pr = row.get("pass_rate").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let n_scored = row.get("n_scored").and_then(|x| x.as_u64()).unwrap_or(0);
            let n_skip = row.get("n_skip").and_then(|x| x.as_u64()).unwrap_or(0);
            let n_err = row.get("n_error").and_then(|x| x.as_u64()).unwrap_or(0);
            board.push_str(&format!(
                "<tr><td class=\"eval-hname\">{name}</td>\
                 <td>{avg:.2}</td><td>{solid:.2}</td><td>{:.0}%</td>\
                 <td>{n_scored}</td><td>{n_skip}</td><td>{n_err}</td></tr>",
                pr * 100.0
            ));
        }
    }
    board.push_str("</tbody></table>");

    // Task matrix
    let mut matrix = String::new();
    if let Some(tm) = r.get("task_matrix").and_then(|m| m.as_object()) {
        let mut harness_names: Vec<String> = Vec::new();
        if let Some(hs) = harnesses {
            harness_names = hs.keys().cloned().collect();
        }
        matrix.push_str("<h3 class=\"eval-subh\">Per-task</h3><table class=\"eval-board\"><thead><tr><th>Task</th>");
        for h in &harness_names {
            matrix.push_str(&format!("<th>{h}</th>"));
        }
        matrix.push_str("</tr></thead><tbody>");
        for (task, row) in tm {
            matrix.push_str(&format!("<td class=\"eval-hname\">{task}</td>"));
            // fix: need <tr>
            // rebuild properly below
            let _ = (task, row);
        }
        // rebuild cleanly
        matrix.clear();
        matrix.push_str(
            "<h3 class=\"eval-subh\">Per-task scores</h3><div class=\"eval-matrix-wrap\"><table class=\"eval-board\"><thead><tr><th>Task</th>",
        );
        for h in &harness_names {
            matrix.push_str(&format!("<th>{h}</th>"));
        }
        matrix.push_str("</tr></thead><tbody>");
        for (task, row) in tm {
            matrix.push_str(&format!("<tr><td class=\"eval-hname\">{task}</td>"));
            for h in &harness_names {
                let cell = row
                    .get(h)
                    .and_then(|x| x.as_f64())
                    .map(|v| format!("{v:.2}"))
                    .unwrap_or_else(|| "—".into());
                matrix.push_str(&format!("<td>{cell}</td>"));
            }
            matrix.push_str("</tr>");
        }
        matrix.push_str("</tbody></table></div>");
    }

    // Item list (compact)
    let mut items = String::from("<ul class=\"eval-cases\">");
    if let Some(arr) = r.get("items").and_then(|c| c.as_array()) {
        for it in arr {
            let h = it.get("harness").and_then(|x| x.as_str()).unwrap_or("?");
            let tid = it.get("task_id").and_then(|x| x.as_str()).unwrap_or("?");
            let st = it.get("status").and_then(|x| x.as_str()).unwrap_or("?");
            let sc = it.get("score").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let (cls, m) = match st {
                "pass" => ("ok", "✓"),
                "fail" => ("bad", "✗"),
                "skip" => ("skip", "○"),
                _ => ("bad", "!"),
            };
            items.push_str(&format!(
                "<li class=\"eval-case {cls}\"><span class=\"eval-case-mark\">{m}</span> \
                 {h}/{tid} <span class=\"eval-case-st\">{st}</span> {sc:.2}</li>"
            ));
        }
    }
    items.push_str("</ul>");

    card.set_inner_html(&format!(
        "<div class=\"eval-result-head\">\
           <span class=\"eval-badge compare large\">CMP</span>\
           <div class=\"eval-result-main\">\
             <div class=\"eval-result-title\">Harness compare</div>\
             <div class=\"eval-result-meta\">{rid} · scored {correct}/{total}</div>\
           </div>\
           <div class=\"eval-result-side\">\
             <div class=\"eval-result-when\">{wh}</div>\
           </div>\
         </div>\
         <h3 class=\"eval-subh\">Leaderboard</h3>{board}{matrix}{items}",
        wh = ago(r.get("created").and_then(|x| x.as_str())),
    ));
    Ok(())
}

fn paint_catalog(card: &Element, r: &Value) -> Result<(), JsValue> {
    let rid = r.get("id").and_then(|x| x.as_str()).unwrap_or("catalog");
    let sel_n = r
        .get("selection")
        .and_then(|s| s.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let s = r.get("summary");
    let n_pass = s
        .and_then(|x| x.get("correct"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let n_scored = s
        .and_then(|x| x.get("n_scored"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let n_skip = s
        .and_then(|x| x.get("n_skip"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let acc = s
        .and_then(|x| x.get("accuracy"))
        .and_then(|x| x.as_f64())
        .unwrap_or(0.0);

    let mut by_src = String::from(
        "<h3 class=\"eval-subh\">By source</h3><table class=\"eval-board\"><thead><tr>\
         <th>Source</th><th>pass</th><th>fail</th><th>skip</th><th>err</th><th>acc</th></tr></thead><tbody>",
    );
    if let Some(obj) = r.get("by_source").and_then(|x| x.as_object()) {
        for (name, row) in obj {
            by_src.push_str(&format!(
                "<tr><td class=\"eval-hname\">{name}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{:.0}%</td></tr>",
                row.get("n_pass").and_then(|x| x.as_u64()).unwrap_or(0),
                row.get("n_fail").and_then(|x| x.as_u64()).unwrap_or(0),
                row.get("n_skip").and_then(|x| x.as_u64()).unwrap_or(0),
                row.get("n_error").and_then(|x| x.as_u64()).unwrap_or(0),
                row.get("accuracy").and_then(|x| x.as_f64()).unwrap_or(0.0) * 100.0
            ));
        }
    }
    by_src.push_str("</tbody></table>");

    let mut by_h = String::from(
        "<h3 class=\"eval-subh\">By harness</h3><table class=\"eval-board\"><thead><tr>\
         <th>Harness</th><th>pass</th><th>fail</th><th>skip</th><th>err</th><th>acc</th></tr></thead><tbody>",
    );
    if let Some(obj) = r.get("by_harness").and_then(|x| x.as_object()) {
        for (name, row) in obj {
            by_h.push_str(&format!(
                "<tr><td class=\"eval-hname\">{name}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{:.0}%</td></tr>",
                row.get("n_pass").and_then(|x| x.as_u64()).unwrap_or(0),
                row.get("n_fail").and_then(|x| x.as_u64()).unwrap_or(0),
                row.get("n_skip").and_then(|x| x.as_u64()).unwrap_or(0),
                row.get("n_error").and_then(|x| x.as_u64()).unwrap_or(0),
                row.get("accuracy").and_then(|x| x.as_f64()).unwrap_or(0.0) * 100.0
            ));
        }
    }
    by_h.push_str("</tbody></table>");

    let mut items = String::from("<ul class=\"eval-cases\">");
    if let Some(arr) = r.get("items").and_then(|c| c.as_array()) {
        for it in arr.iter().take(40) {
            let h = it.get("harness").and_then(|x| x.as_str()).unwrap_or("?");
            let id = it.get("full_id").and_then(|x| x.as_str()).unwrap_or("?");
            let st = it.get("status").and_then(|x| x.as_str()).unwrap_or("?");
            let sc = it.get("score").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let (cls, m) = match st {
                "pass" => ("ok", "✓"),
                "fail" => ("bad", "✗"),
                "skip" => ("skip", "○"),
                _ => ("bad", "!"),
            };
            items.push_str(&format!(
                "<li class=\"eval-case {cls}\"><span class=\"eval-case-mark\">{m}</span> {h}/{id} {st} {sc:.2}</li>"
            ));
        }
    }
    items.push_str("</ul>");

    card.set_inner_html(&format!(
        "<div class=\"eval-result-head\">\
           <span class=\"eval-badge compare large\">CAT</span>\
           <div class=\"eval-result-main\">\
             <div class=\"eval-result-title\">Catalog sample</div>\
             <div class=\"eval-result-meta\">{rid} · {sel_n} items · scored {n_pass}/{n_scored} · skip {n_skip} · {:.0}%</div>\
           </div>\
           <div class=\"eval-result-side\"><div class=\"eval-result-when\">{wh}</div></div>\
         </div>{by_src}{by_h}{items}",
        acc * 100.0,
        wh = ago(r.get("created").and_then(|x| x.as_str())),
    ));
    Ok(())
}

async fn run(
    document: &Document,
    list: &Element,
    suite: &str,
    agent: Option<String>,
) -> Result<(), JsValue> {
    status(document, &format!("Running {suite}…"), true);
    busy(document, true);
    let mut body = serde_json::json!({ "suite": suite });
    if let Some(a) = agent {
        body["agent_id"] = Value::String(a);
    }
    if suite == "catalog" {
        // Empty harnesses → server discovers all enabled evals/vendors
        body["sample_n"] = serde_json::json!(20);
        body["seed"] = serde_json::json!(42);
    }
    if suite == "compare" {
        // Legacy local tasks only
        body["harnesses"] = Value::String("jewell,hermes".into());
        body["tasks"] = Value::String("all".into());
    }
    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::Cors);
    opts.set_body(&JsValue::from_str(&body.to_string()));
    let req = Request::new_with_str_and_init("/api/evals/run/stream", &opts)?;
    req.headers().set("content-type", "application/json")?;
    let resp: Response = JsFuture::from(web_sys::window().unwrap().fetch_with_request(&req))
        .await?
        .dyn_into()?;
    if !resp.ok() {
        status(document, "Run failed to start", true);
        busy(document, false);
        return Ok(());
    }
    let text = JsFuture::from(resp.text()?)
        .await?
        .as_string()
        .unwrap_or_default();
    let mut rid = None::<String>;
    for line in text.lines() {
        let Some(data) = line.trim().strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("log") => {
                if let Some(m) = v.get("msg").and_then(|m| m.as_str()) {
                    status(document, m, true);
                }
            }
            Some("error") => status(
                document,
                &format!(
                    "Error: {}",
                    v.get("msg").and_then(|m| m.as_str()).unwrap_or("error")
                ),
                true,
            ),
            Some("done") => {
                if let Some(r) = v.get("report") {
                    if let Some(card) = document.get_element_by_id("eval-result") {
                        let _ = paint(&card, r);
                    }
                    rid = r.get("id").and_then(|x| x.as_str()).map(|s| s.to_string());
                }
                status(document, "Done", true);
            }
            _ => {}
        }
    }
    busy(document, false);
    let _ = reload(document, list, rid.as_deref()).await;
    status(document, "", false);
    Ok(())
}

async fn get(url: &str) -> Result<String, JsValue> {
    let resp: Response = JsFuture::from(web_sys::window().unwrap().fetch_with_str(url))
        .await?
        .dyn_into()?;
    Ok(JsFuture::from(resp.text()?)
        .await?
        .as_string()
        .unwrap_or_default())
}

fn status(document: &Document, msg: &str, show: bool) {
    let Some(el) = document.get_element_by_id("eval-status") else {
        return;
    };
    if show && !msg.is_empty() {
        el.set_text_content(Some(msg));
        let _ = el.remove_attribute("hidden");
    } else {
        el.set_text_content(Some(""));
        let _ = el.set_attribute("hidden", "");
    }
}

fn busy(document: &Document, on: bool) {
    for id in [
        "btn-run-team",
        "btn-run-agent",
        "btn-run-catalog",
        "btn-run-compare",
    ] {
        if let Some(el) = document.get_element_by_id(id) {
            if let Ok(btn) = el.dyn_into::<HtmlButtonElement>() {
                btn.set_disabled(on);
            }
        }
    }
}

fn sel(document: &Document, id: &str) -> Option<String> {
    document
        .get_element_by_id(id)
        .and_then(|el| el.dyn_into::<HtmlSelectElement>().ok())
        .map(|s| s.value())
}

fn score(run: &Value) -> String {
    match (
        run.get("correct").and_then(|x| x.as_u64()),
        run.get("total").and_then(|x| x.as_u64()),
    ) {
        (Some(c), Some(t)) if t > 0 => format!("{c}/{t}"),
        _ => run
            .get("accuracy")
            .and_then(|x| x.as_f64())
            .map(|a| format!("{:.0}%", a * 100.0))
            .unwrap_or_else(|| "—".into()),
    }
}

fn ago(iso: Option<&str>) -> String {
    let Some(iso) = iso else {
        return String::new();
    };
    let ms = js_sys::Date::new(&JsValue::from_str(iso)).get_time();
    if !ms.is_finite() {
        return String::new();
    }
    let secs = ((js_sys::Date::now() - ms) / 1000.0).floor() as i64;
    if secs < 60 {
        "just now".into()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}
