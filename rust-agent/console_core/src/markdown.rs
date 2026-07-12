//! Minimal markdown → HTML for assistant bubbles (no external deps).

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn inline_md(s: &str) -> String {
    let mut t = escape_html(s);
    // code
    t = regex_replace_code(&t);
    // bold
    t = bold_replace(&t);
    t
}

fn regex_replace_code(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find('`') {
        out.push_str(&rest[..start]);
        rest = &rest[start + 1..];
        if let Some(end) = rest.find('`') {
            out.push_str("<code>");
            out.push_str(&rest[..end]);
            out.push_str("</code>");
            rest = &rest[end + 1..];
        } else {
            out.push('`');
            out.push_str(rest);
            return out;
        }
    }
    out.push_str(rest);
    out
}

fn bold_replace(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("**") {
        out.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        if let Some(end) = rest.find("**") {
            out.push_str("<strong>");
            out.push_str(&rest[..end]);
            out.push_str("</strong>");
            rest = &rest[end + 2..];
        } else {
            out.push_str("**");
            out.push_str(rest);
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// Render a subset of markdown to HTML (lists, headings, fences, bold, code).
pub fn render_markdown(src: &str) -> String {
    if src.is_empty() {
        return String::new();
    }
    let normalized = src.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut in_code = false;
    let mut code_lang = String::new();
    let mut code_buf: Vec<&str> = Vec::new();
    let mut list_type: Option<&str> = None;
    let mut para: Vec<String> = Vec::new();

    let flush_para = |para: &mut Vec<String>, out: &mut Vec<String>| {
        if para.is_empty() {
            return;
        }
        out.push(format!("<p>{}</p>", inline_md(&para.join(" "))));
        para.clear();
    };
    let flush_list = |list_type: &mut Option<&str>, out: &mut Vec<String>| {
        if let Some(t) = list_type.take() {
            out.push(format!("</{t}>"));
        }
    };
    let flush_code =
        |in_code: &mut bool, code_lang: &mut String, code_buf: &mut Vec<&str>, out: &mut Vec<String>| {
            if !*in_code {
                return;
            }
            let body = escape_html(&code_buf.join("\n"));
            let cls = if code_lang.is_empty() {
                String::new()
            } else {
                format!(" class=\"lang-{}\"", escape_html(code_lang))
            };
            out.push(format!("<pre class=\"md-code\"><code{cls}>{body}</code></pre>"));
            *in_code = false;
            code_lang.clear();
            code_buf.clear();
        };

    while i < lines.len() {
        let line = lines[i];
        if let Some(rest) = line.strip_prefix("```") {
            flush_para(&mut para, &mut out);
            flush_list(&mut list_type, &mut out);
            if in_code {
                flush_code(&mut in_code, &mut code_lang, &mut code_buf, &mut out);
            } else {
                in_code = true;
                code_lang = rest.trim().to_string();
                code_buf.clear();
            }
            i += 1;
            continue;
        }
        if in_code {
            code_buf.push(line);
            i += 1;
            continue;
        }
        if line.trim().is_empty() {
            flush_para(&mut para, &mut out);
            flush_list(&mut list_type, &mut out);
            i += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix("### ") {
            flush_para(&mut para, &mut out);
            flush_list(&mut list_type, &mut out);
            out.push(format!("<h3>{}</h3>", inline_md(rest)));
            i += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix("## ") {
            flush_para(&mut para, &mut out);
            flush_list(&mut list_type, &mut out);
            out.push(format!("<h2>{}</h2>", inline_md(rest)));
            i += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix("# ") {
            flush_para(&mut para, &mut out);
            flush_list(&mut list_type, &mut out);
            out.push(format!("<h1>{}</h1>", inline_md(rest)));
            i += 1;
            continue;
        }
        let ul = line.trim_start().strip_prefix("- ").or_else(|| line.trim_start().strip_prefix("* "));
        if let Some(item) = ul {
            flush_para(&mut para, &mut out);
            if list_type != Some("ul") {
                flush_list(&mut list_type, &mut out);
                list_type = Some("ul");
                out.push("<ul>".into());
            }
            out.push(format!("<li>{}</li>", inline_md(item)));
            i += 1;
            continue;
        }
        // ordered list "1. item"
        if let Some(dot) = line.trim_start().find(". ") {
            let (num, rest) = line.trim_start().split_at(dot);
            if num.chars().all(|c| c.is_ascii_digit()) {
                flush_para(&mut para, &mut out);
                if list_type != Some("ol") {
                    flush_list(&mut list_type, &mut out);
                    list_type = Some("ol");
                    out.push("<ol>".into());
                }
                out.push(format!("<li>{}</li>", inline_md(&rest[2..])));
                i += 1;
                continue;
            }
        }
        flush_list(&mut list_type, &mut out);
        para.push(line.trim().to_string());
        i += 1;
    }
    flush_code(&mut in_code, &mut code_lang, &mut code_buf, &mut out);
    flush_para(&mut para, &mut out);
    flush_list(&mut list_type, &mut out);
    if out.is_empty() {
        format!("<p>{}</p>", inline_md(src))
    } else {
        out.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_list_and_escape() {
        let html = render_markdown("**Tools**\n\n- list_tools\n- app_status\n");
        assert!(html.contains("<strong>Tools</strong>"));
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>list_tools</li>"));
        let esc = render_markdown("```yaml\nname: <evil>\n```");
        assert!(esc.contains("&lt;evil&gt;"));
        assert!(!esc.contains("<evil>"));
    }
}
