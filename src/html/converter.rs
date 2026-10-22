use std::collections::HashMap;

// ------------------------------------------------------------------
// Public API
// ------------------------------------------------------------------

pub fn html_to_markdown(html: &str) -> String {
    if html.trim().is_empty() {
        return String::new();
    }
    let html = html.replace("\r\n", "\n").replace('\r', "\n");
    let dom = parse(&html);
    let kids = dom.nodes[0].children.clone();
    let md = blocks_of(&dom, &kids, false);
    format!("{}\n", md.trim_start_matches('\n').trim_end())
}

/// Reads `&amp;`, `&#65;`, `&#x41;` style entities.
pub fn decode_html_entities(s: &str) -> String {
    decode(s)
}

// ------------------------------------------------------------------
// Data
// ------------------------------------------------------------------

/// Private marker for a line break
const BR: char = '\u{1}';

struct Node {
    name: Option<String>, // None means text node
    text: String,
    attrs: HashMap<String, String>,
    children: Vec<usize>,
    parent: Option<usize>,
}

struct Dom {
    nodes: Vec<Node>,
}

impl Dom {
    fn add(&mut self, parent: usize, name: Option<String>, text: String, attrs: HashMap<String, String>) -> usize {
        let id = self.nodes.len();
        self.nodes.push(Node { name, text, attrs, children: Vec::new(), parent: Some(parent) });
        self.nodes[parent].children.push(id);
        id
    }

    fn name(&self, id: usize) -> Option<&str> {
        self.nodes[id].name.as_deref()
    }

    fn attr(&self, id: usize, key: &str) -> Option<&str> {
        self.nodes[id].attrs.get(key).map(|s| s.as_str())
    }

    fn has(&self, id: usize, key: &str) -> bool {
        self.nodes[id].attrs.contains_key(key)
    }
}

struct Part {
    text: String,
    kind: char,   // 'u' = ul, 'o' = ol, ' ' = other
    inline: bool, // true if it came from loose inline text
}

fn is_void(name: &str) -> bool {
    matches!(
        name,
        "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" | "link" | "meta" | "source" | "track" | "wbr"
    )
}

fn is_skip_tag(name: &str) -> bool {
    matches!(
        name,
        "script" | "style" | "head" | "title" | "template" | "noscript" | "svg" | "canvas" | "iframe" | "meta" | "link"
    )
}

fn is_block(name: &str) -> bool {
    matches!(
        name,
        "address" | "article" | "aside" | "blockquote" | "body" | "center" | "dd" | "details" | "div" | "dl" | "dt"
            | "fieldset" | "figcaption" | "figure" | "footer" | "form" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            | "header" | "hgroup" | "hr" | "html" | "li" | "main" | "nav" | "ol" | "p" | "pre" | "section"
            | "summary" | "table" | "ul"
    )
}

fn entity(name: &str) -> Option<&'static str> {
    Some(match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{00A0}",
        "copy" => "\u{00A9}",
        "reg" => "\u{00AE}",
        "trade" => "\u{2122}",
        "hellip" => "\u{2026}",
        "mdash" => "\u{2014}",
        "ndash" => "\u{2013}",
        "lsquo" => "\u{2018}",
        "rsquo" => "\u{2019}",
        "ldquo" => "\u{201C}",
        "rdquo" => "\u{201D}",
        "laquo" => "\u{00AB}",
        "raquo" => "\u{00BB}",
        "bull" => "\u{2022}",
        "middot" => "\u{00B7}",
        "times" => "\u{00D7}",
        "divide" => "\u{00F7}",
        "euro" => "\u{20AC}",
        "pound" => "\u{00A3}",
        "yen" => "\u{00A5}",
        "cent" => "\u{00A2}",
        "deg" => "\u{00B0}",
        "plusmn" => "\u{00B1}",
        "para" => "\u{00B6}",
        "sect" => "\u{00A7}",
        "larr" => "\u{2190}",
        "rarr" => "\u{2192}",
        "uarr" => "\u{2191}",
        "darr" => "\u{2193}",
        "harr" => "\u{2194}",
        "hearts" => "\u{2665}",
        "frac12" => "\u{00BD}",
        "frac14" => "\u{00BC}",
        "frac34" => "\u{00BE}",
        "ne" => "\u{2260}",
        "le" => "\u{2264}",
        "ge" => "\u{2265}",
        "infin" => "\u{221E}",
        "ensp" => "\u{2002}",
        "emsp" => "\u{2003}",
        "thinsp" => "\u{2009}",
        _ => return None,
    })
}

fn skip(dom: &Dom, id: usize) -> bool {
    match dom.name(id) {
        Some(n) => is_skip_tag(n) || dom.has(id, "hidden"),
        None => false,
    }
}

// ------------------------------------------------------------------
// Small string helpers
// ------------------------------------------------------------------

fn find_char(c: &[char], from: usize, ch: char) -> Option<usize> {
    if from >= c.len() {
        return None;
    }
    c[from..].iter().position(|&x| x == ch).map(|p| p + from)
}

fn starts_at(c: &[char], i: usize, s: &str) -> bool {
    let mut k = i;
    for ch in s.chars() {
        if k >= c.len() || c[k] != ch {
            return false;
        }
        k += 1;
    }
    true
}

fn find_str(c: &[char], from: usize, needle: &str) -> Option<usize> {
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() || c.len() < n.len() {
        return None;
    }
    let mut i = from;
    while i + n.len() <= c.len() {
        if c[i..i + n.len()] == n[..] {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Case-insensitive (ASCII) search.
fn find_ci(c: &[char], from: usize, needle: &str) -> Option<usize> {
    let n: Vec<char> = needle.chars().map(|x| x.to_ascii_lowercase()).collect();
    if n.is_empty() || c.len() < n.len() {
        return None;
    }
    let mut i = from;
    while i + n.len() <= c.len() {
        if (0..n.len()).all(|k| c[i + k].to_ascii_lowercase() == n[k]) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn starts_with_ci(s: &str, prefix: &str) -> bool {
    s.to_ascii_lowercase().starts_with(prefix)
}

fn is_space(c: char) -> bool {
    c == ' ' || c == '\n' || c == BR
}

fn is_edge(c: char) -> bool {
    c == ' ' || c == '\n' || c == '\t' || c == BR
}

fn trim_edge(s: &str) -> &str {
    s.trim_matches(is_edge)
}

fn last_char(s: &str) -> Option<char> {
    s.chars().next_back()
}

/// Appends already-escaped text. Drops a leading space if there is one already.
fn put(sb: &mut String, s: &str) {
    if s.is_empty() {
        return;
    }
    if s.starts_with(' ') && last_char(sb).map_or(false, is_space) {
        sb.push_str(&s[1..]);
    } else {
        sb.push_str(s);
    }
}

fn has_space(c: &[char], from: usize, to: usize) -> bool {
    c[from..to].iter().any(|&x| x == ' ' || x == '\n')
}

/// Splits "  text  " into one space of lead, the core, and one space of trail.
fn edge(s: &str) -> (&'static str, String, &'static str) {
    let c: Vec<char> = s.chars().collect();
    let mut a = 0;
    let mut b = c.len();
    while a < b && is_edge(c[a]) {
        a += 1;
    }
    while b > a && is_edge(c[b - 1]) {
        b -= 1;
    }
    let core: String = c[a..b].iter().collect();
    let lead = if has_space(&c, 0, a) { " " } else { "" };
    let trail = if has_space(&c, b, c.len()) { " " } else { "" };
    (lead, core, trail)
}

/// Replaces every run of spaces and BR markers that holds at least one BR.
/// The closure gets the number of BR markers in the run.
fn replace_breaks<F: Fn(usize) -> String>(s: &str, f: F) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] == ' ' || c[i] == BR {
            let start = i;
            while i < c.len() && (c[i] == ' ' || c[i] == BR) {
                i += 1;
            }
            let count = c[start..i].iter().filter(|&&x| x == BR).count();
            if count > 0 {
                out.push_str(&f(count));
            } else {
                out.extend(c[start..i].iter());
            }
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

/// Turns loose inline text into a paragraph. One break = hard line break, many = new paragraph.
fn paragraph(s: &str) -> String {
    let s = trim_edge(s);
    if s.is_empty() {
        return String::new();
    }
    replace_breaks(s, |count| if count == 1 { "\\\n".to_string() } else { "\n\n".to_string() })
}

/// Same, but for places that must stay on one line (headings, table cells, link text).
fn one_line(s: &str, br: &str) -> String {
    let s = trim_edge(s);
    replace_breaks(s, |_| br.to_string())
}

/// HTML treats any run of space, tab, newline as one space.
fn collapse_ws(s: &str) -> String {
    let mut sb = String::with_capacity(s.len());
    let mut space = false;
    for c in s.chars() {
        if c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\u{c}' {
            space = true;
            continue;
        }
        if space {
            sb.push(' ');
            space = false;
        }
        sb.push(c);
    }
    if space {
        sb.push(' ');
    }
    sb
}

/// Puts a backslash before characters that Markdown would read as markup.
fn escape(t: &str) -> String {
    let c: Vec<char> = t.chars().collect();
    let mut sb = String::with_capacity(t.len() + 8);
    for i in 0..c.len() {
        let ch = c[i];
        match ch {
            '\\' | '`' | '*' | '[' | ']' | '<' | '~' => {
                sb.push('\\');
                sb.push(ch);
            }
            '_' => {
                let in_word =
                    i > 0 && i + 1 < c.len() && c[i - 1].is_alphanumeric() && c[i + 1].is_alphanumeric();
                if !in_word {
                    sb.push('\\');
                }
                sb.push(ch);
            }
            '&' => {
                if looks_like_entity(&c, i) {
                    sb.push('\\');
                }
                sb.push(ch);
            }
            _ => sb.push(ch),
        }
    }

    let r: Vec<char> = sb.chars().collect();
    if r.is_empty() {
        return sb;
    }

    // Things that would turn a line into a heading, quote, or list.
    let f = r[0];
    if f == '#' || f == '>' {
        return format!("\\{}", sb);
    }
    if (f == '-' || f == '+') && (r.len() == 1 || r[1] == ' ' || r[1] == '-') {
        return format!("\\{}", sb);
    }
    if f.is_ascii_digit() {
        let mut k = 0;
        while k < r.len() && k < 9 && r[k].is_ascii_digit() {
            k += 1;
        }
        if k < r.len() && (r[k] == '.' || r[k] == ')') && (k + 1 == r.len() || r[k + 1] == ' ') {
            let head: String = r[..k].iter().collect();
            let tail: String = r[k..].iter().collect();
            return format!("{}\\{}", head, tail);
        }
    }
    sb
}

fn looks_like_entity(t: &[char], i: usize) -> bool {
    let mut j = i + 1;
    if j < t.len() && t[j] == '#' {
        j += 1;
    }
    let start = j;
    while j < t.len() && j - i < 12 && t[j].is_alphanumeric() {
        j += 1;
    }
    j > start && j < t.len() && t[j] == ';'
}

fn decode(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let c: Vec<char> = s.chars().collect();
    let mut sb = String::with_capacity(s.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] == '&' {
            if let Some(semi) = find_char(&c, i + 1, ';') {
                if semi > i + 1 && semi - i <= 10 {
                    let ent: String = c[i + 1..semi].iter().collect();
                    let mut rep: Option<String> = None;
                    if ent.starts_with('#') {
                        let e: Vec<char> = ent.chars().collect();
                        let cp: Option<u32> = if e.len() > 2 && (e[1] == 'x' || e[1] == 'X') {
                            let h: String = e[2..].iter().collect();
                            if h.chars().all(|x| x.is_ascii_hexdigit()) {
                                u32::from_str_radix(&h, 16).ok()
                            } else {
                                None
                            }
                        } else {
                            let d: String = e[1..].iter().collect();
                            if !d.is_empty() && d.chars().all(|x| x.is_ascii_digit()) {
                                d.parse::<u32>().ok()
                            } else {
                                None
                            }
                        };
                        if let Some(cp) = cp {
                            if cp > 0 {
                                // from_u32 also refuses > 0x10FFFF and surrogates
                                rep = char::from_u32(cp).map(|x| x.to_string());
                            }
                        }
                    } else {
                        rep = entity(&ent).map(|x| x.to_string());
                    }
                    if let Some(r) = rep {
                        sb.push_str(&r);
                        i = semi + 1;
                        continue;
                    }
                }
            }
        }
        sb.push(c[i]);
        i += 1;
    }
    sb
}

fn longest_run(s: &str, ch: char) -> usize {
    let mut max = 0;
    let mut run = 0;
    for c in s.chars() {
        if c == ch {
            run += 1;
            if run > max {
                max = run;
            }
        } else {
            run = 0;
        }
    }
    max
}

fn code_span(code: &str) -> String {
    let fence = "`".repeat(longest_run(code, '`') + 1);
    let pad = code.starts_with('`') || code.ends_with('`');
    if pad {
        format!("{} {} {}", fence, code, fence)
    } else {
        format!("{}{}{}", fence, code, fence)
    }
}

fn title_part(title: Option<&str>) -> String {
    match title {
        Some(t) if !t.trim().is_empty() => {
            let t = collapse_ws(t).trim().replace('\\', "\\\\").replace('"', "\\\"");
            format!(" \"{}\"", t)
        }
        _ => String::new(),
    }
}

fn dest(url: &str) -> String {
    url.trim()
        .replace('\n', "")
        .replace(' ', "%20")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace('<', "%3C")
        .replace('>', "%3E")
}

fn indent(text: &str, pad: &str, include_first: bool) -> String {
    let mut lines: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
    let start = if include_first { 0 } else { 1 };
    for line in lines.iter_mut().skip(start) {
        if !line.is_empty() {
            *line = format!("{}{}", pad, line);
        }
    }
    lines.join("\n")
}

// ------------------------------------------------------------------
// Step 1: a small forgiving HTML parser
// ------------------------------------------------------------------

fn flush_text(dom: &mut Dom, cur: usize, text: &mut String) {
    if text.is_empty() {
        return;
    }
    let t = std::mem::take(text);
    dom.add(cur, None, t, HashMap::new());
}

fn parse(html: &str) -> Dom {
    let c: Vec<char> = html.chars().collect();
    let n = c.len();

    let mut dom = Dom {
        nodes: vec![Node {
            name: Some("#root".to_string()),
            text: String::new(),
            attrs: HashMap::new(),
            children: Vec::new(),
            parent: None,
        }],
    };
    let root = 0usize;
    let mut cur = root;
    let mut text = String::new();
    let mut i = 0usize;

    while i < n {
        if c[i] != '<' {
            let next = find_char(&c, i, '<').unwrap_or(n);
            text.extend(c[i..next].iter());
            i = next;
            continue;
        }

        if starts_at(&c, i, "<!--") {
            i = match find_str(&c, i + 4, "-->") {
                Some(e) => e + 3,
                None => n,
            };
            continue;
        }

        if i + 1 < n && (c[i + 1] == '!' || c[i + 1] == '?') {
            i = match find_char(&c, i, '>') {
                Some(e) => e + 1,
                None => n,
            };
            continue;
        }

        let closing = i + 1 < n && c[i + 1] == '/';
        let mut p = i + if closing { 2 } else { 1 };
        if p >= n || !c[p].is_alphabetic() {
            text.push('<');
            i += 1;
            continue;
        }

        let name_start = p;
        while p < n && (c[p].is_alphanumeric() || c[p] == '-' || c[p] == ':') {
            p += 1;
        }
        let name: String = c[name_start..p].iter().collect::<String>().to_lowercase();

        if closing {
            i = match find_char(&c, p, '>') {
                Some(e) => e + 1,
                None => n,
            };
            flush_text(&mut dom, cur, &mut text);
            let mut x = Some(cur);
            while let Some(xi) = x {
                if xi == root {
                    break;
                }
                if dom.nodes[xi].name.as_deref() == Some(name.as_str()) {
                    cur = dom.nodes[xi].parent.unwrap_or(root);
                    break;
                }
                x = dom.nodes[xi].parent;
            }
            continue;
        }

        // Read attributes.
        let mut attrs: HashMap<String, String> = HashMap::new();
        let mut self_close = false;
        while p < n {
            while p < n && c[p].is_whitespace() {
                p += 1;
            }
            if p >= n {
                break;
            }
            if c[p] == '>' {
                p += 1;
                break;
            }
            if c[p] == '/' {
                if p + 1 < n && c[p + 1] == '>' {
                    self_close = true;
                    p += 2;
                    break;
                }
                p += 1;
                continue;
            }
            let an = p;
            while p < n && !c[p].is_whitespace() && c[p] != '=' && c[p] != '>' && c[p] != '/' {
                p += 1;
            }
            if p == an {
                p += 1;
                continue;
            }
            let attr_name: String = c[an..p].iter().collect::<String>().to_lowercase();
            let mut attr_value = String::new();
            let mut q = p;
            while q < n && c[q].is_whitespace() {
                q += 1;
            }
            if q < n && c[q] == '=' {
                q += 1;
                while q < n && c[q].is_whitespace() {
                    q += 1;
                }
                if q < n && (c[q] == '"' || c[q] == '\'') {
                    let quote = c[q];
                    let close = find_char(&c, q + 1, quote).unwrap_or(n);
                    attr_value = c[q + 1..close].iter().collect();
                    q = (close + 1).min(n);
                } else {
                    let vs = q;
                    while q < n && !c[q].is_whitespace() && c[q] != '>' {
                        q += 1;
                    }
                    attr_value = c[vs..q].iter().collect();
                }
                p = q;
            }
            attrs.insert(attr_name, decode(&attr_value));
        }
        i = p;
        flush_text(&mut dom, cur, &mut text);

        // Skip the inside of script and style completely.
        if name == "script" || name == "style" {
            let needle = format!("</{}", name);
            match find_ci(&c, i, &needle) {
                None => i = n,
                Some(close) => {
                    i = match find_char(&c, close, '>') {
                        Some(gt) => gt + 1,
                        None => n,
                    };
                }
            }
            continue;
        }

        cur = auto_close(&dom, cur, &name);
        let id = dom.add(cur, Some(name.clone()), String::new(), attrs);
        if !self_close && !is_void(&name) {
            cur = id;
        }
    }

    flush_text(&mut dom, cur, &mut text);
    dom
}

/// Closes tags that HTML closes by itself (like <li> after <li>).
fn auto_close(dom: &Dom, cur: usize, name: &str) -> usize {
    match name {
        "li" => close_up(dom, cur, &["li"], &["ul", "ol"]),
        "dt" | "dd" => close_up(dom, cur, &["dt", "dd"], &["dl"]),
        "td" | "th" => close_up(dom, cur, &["td", "th"], &["tr", "table"]),
        "tr" => close_up(dom, cur, &["tr"], &["table", "thead", "tbody", "tfoot"]),
        "thead" | "tbody" | "tfoot" => close_up(dom, cur, &["thead", "tbody", "tfoot"], &["table"]),
        _ => {
            if is_block(name) && dom.name(cur) == Some("p") {
                if let Some(p) = dom.nodes[cur].parent {
                    return p;
                }
            }
            cur
        }
    }
}

fn close_up(dom: &Dom, cur: usize, targets: &[&str], stops: &[&str]) -> usize {
    let mut x = Some(cur);
    while let Some(xi) = x {
        if let Some(nm) = dom.name(xi) {
            if stops.contains(&nm) {
                break;
            }
            if targets.contains(&nm) {
                return dom.nodes[xi].parent.unwrap_or(cur);
            }
        }
        x = dom.nodes[xi].parent;
    }
    cur
}

// ------------------------------------------------------------------
// Step 2: block level output
// ------------------------------------------------------------------

/// Renders a list of nodes as Markdown blocks.
/// `tight` is used inside list items so nested lists stay close to their text.
fn blocks_of(dom: &Dom, kids: &[usize], tight: bool) -> String {
    let mut parts: Vec<Part> = Vec::new();
    let mut run = String::new();

    for &c in kids {
        if skip(dom, c) {
            continue;
        }
        match dom.name(c) {
            Some(nm) if is_block(nm) => {
                flush_run(&mut run, &mut parts);
                let b = render_block(dom, c);
                if !b.is_empty() {
                    let kind = match nm {
                        "ul" => 'u',
                        "ol" => 'o',
                        _ => ' ',
                    };
                    parts.push(Part { text: b, kind, inline: false });
                }
            }
            _ => render_inline(dom, c, &mut run),
        }
    }
    flush_run(&mut run, &mut parts);

    let mut sb = String::new();
    for k in 0..parts.len() {
        if k > 0 {
            let prev = &parts[k - 1];
            let cur = &parts[k];
            if cur.kind != ' ' && cur.kind == prev.kind {
                sb.push_str("\n\n<!-- -->\n\n"); // keep two lists apart
            } else if tight && cur.kind != ' ' && prev.inline {
                sb.push('\n');
            } else {
                sb.push_str("\n\n");
            }
        }
        sb.push_str(&parts[k].text);
    }
    sb
}

fn flush_run(run: &mut String, parts: &mut Vec<Part>) {
    let t = paragraph(run);
    run.clear();
    if !t.is_empty() {
        parts.push(Part { text: t, kind: ' ', inline: true });
    }
}

fn render_block(dom: &Dom, id: usize) -> String {
    let name = dom.name(id).unwrap_or("");
    match name {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let mut sb = String::new();
            for &c in &dom.nodes[id].children {
                render_inline(dom, c, &mut sb);
            }
            let t = one_line(&sb, " ");
            if t.is_empty() {
                String::new()
            } else {
                let level = (name.as_bytes()[1] - b'0') as usize;
                format!("{} {}", "#".repeat(level), t)
            }
        }
        "hr" => "---".to_string(),
        "pre" => code_block(dom, id),
        "ul" | "ol" => render_list(dom, id),
        "table" => render_table(dom, id),
        "details" => details(dom, id),
        "blockquote" => {
            let inner = blocks_of(dom, &dom.nodes[id].children, false);
            if inner.is_empty() {
                return String::new();
            }
            inner
                .split('\n')
                .map(|l| if l.is_empty() { ">".to_string() } else { format!("> {}", l) })
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => blocks_of(dom, &dom.nodes[id].children, false),
    }
}

fn code_block(dom: &Dom, pre: usize) -> String {
    let mut raw = String::new();
    raw_text(dom, pre, &mut raw);
    let mut code: &str = &raw;
    if code.starts_with('\n') {
        code = &code[1..];
    }
    let code = code.trim_end_matches('\n');
    if code.trim().is_empty() {
        return String::new();
    }
    let fence = "`".repeat(std::cmp::max(3, longest_run(code, '`') + 1));
    format!("{}{}\n{}\n{}", fence, find_lang(dom, pre), code, fence)
}

fn find_lang(dom: &Dom, pre: usize) -> String {
    let first = dom.nodes[pre].children.iter().copied().find(|&c| dom.name(c).is_some());

    for cand in [Some(pre), first].iter().flatten() {
        let cand = *cand;
        if let Some(cls) = dom.attr(cand, "class") {
            for tok in cls.split(|ch| ch == ' ' || ch == '\t' || ch == '\n') {
                if let Some(rest) = tok.strip_prefix("language-") {
                    return clean_lang(rest);
                }
                if let Some(rest) = tok.strip_prefix("lang-") {
                    return clean_lang(rest);
                }
                if let Some(rest) = tok.strip_prefix("highlight-source-") {
                    return clean_lang(rest);
                }
            }
        }
        if let Some(dl) = dom.attr(cand, "data-lang") {
            if !dl.trim().is_empty() {
                return clean_lang(dl);
            }
        }
    }
    String::new()
}

fn clean_lang(s: &str) -> String {
    s.chars()
        .filter(|&c| c.is_alphanumeric() || c == '-' || c == '+' || c == '#' || c == '.' || c == '_')
        .collect()
}

fn details(dom: &Dom, id: usize) -> String {
    let mut summary = String::new();
    let mut rest: Vec<usize> = Vec::new();
    for &c in &dom.nodes[id].children {
        if dom.name(c) == Some("summary") {
            let mut raw = String::new();
            raw_text(dom, c, &mut raw);
            summary = collapse_ws(&raw)
                .trim()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
        } else {
            rest.push(c);
        }
    }
    let body = blocks_of(dom, &rest, false);

    let mut sb = String::new();
    sb.push_str(if dom.has(id, "open") { "<details open>" } else { "<details>" });
    sb.push('\n');
    if !summary.is_empty() {
        sb.push_str("<summary>");
        sb.push_str(&summary);
        sb.push_str("</summary>\n");
    }
    sb.push('\n');
    if !body.is_empty() {
        sb.push_str(&body);
        sb.push_str("\n\n");
    }
    sb.push_str("</details>");
    sb
}

// ------------------------------------------------------------------
// Lists
// ------------------------------------------------------------------

fn render_list(dom: &Dom, list: usize) -> String {
    let ordered = dom.name(list) == Some("ol");
    let mut num: i64 = 1;
    if ordered {
        if let Some(start) = dom.attr(list, "start").and_then(|s| s.trim().parse::<i64>().ok()) {
            num = start;
        }
    }

    let mut items: Vec<String> = Vec::new();
    let mut last_width = 2usize;

    for &child in &dom.nodes[list].children {
        if dom.name(child).is_none() || skip(dom, child) {
            continue;
        }

        // The children this item will be rendered from.
        let mut kids: Vec<usize> = dom.nodes[child].children.clone();

        let child_name = dom.name(child).unwrap_or("");
        if child_name == "ul" || child_name == "ol" {
            // A list placed directly inside a list: hang it under the item before it.
            let nested = render_list(dom, child);
            if nested.is_empty() {
                continue;
            }
            if let Some(last) = items.last_mut() {
                let pad = " ".repeat(last_width);
                last.push('\n');
                last.push_str(&pad);
                last.push_str(&indent(&nested, &pad, false));
                continue;
            }
            // No item before it: make a fake <li> that holds the list.
            kids = vec![child];
        }

        let marker = if ordered { format!("{}. ", num) } else { "- ".to_string() };
        let mut task = "";
        if let Some(bx) = find_checkbox(dom, &kids) {
            task = if dom.has(bx, "checked") { "[x] " } else { "[ ] " };
        }

        let content = format!("{}{}", task, blocks_of(dom, &kids, true));
        if content.trim().is_empty() {
            items.push(marker.trim_end().to_string());
        } else {
            items.push(format!("{}{}", marker, indent(&content, &" ".repeat(marker.len()), false)));
        }

        last_width = marker.len();
        num += 1;
    }
    items.join("\n")
}

fn find_checkbox(dom: &Dom, kids: &[usize]) -> Option<usize> {
    for &c in kids {
        let name = match dom.name(c) {
            Some(n) => n,
            None => continue,
        };
        if name == "input"
            && dom.attr(c, "type").map_or(false, |t| t.eq_ignore_ascii_case("checkbox"))
        {
            return Some(c);
        }
        if name == "ul" || name == "ol" {
            continue;
        }
        if let Some(r) = find_checkbox(dom, &dom.nodes[c].children) {
            return Some(r);
        }
    }
    None
}

// ------------------------------------------------------------------
// Tables
// ------------------------------------------------------------------

fn render_table(dom: &Dom, table: usize) -> String {
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut caption = String::new();
    collect_rows(dom, table, &mut rows, &mut caption);
    if rows.is_empty() {
        return String::new();
    }

    let mut grid: Vec<Vec<String>> = Vec::new();
    let mut align: Vec<String> = Vec::new();
    let mut cols = 0usize;

    for (r, row) in rows.iter().enumerate() {
        let mut line: Vec<String> = Vec::new();
        for &cell in row {
            let mut sb = String::new();
            for &c in &dom.nodes[cell].children {
                render_inline(dom, c, &mut sb);
            }
            let text = one_line(&sb, "<br>").replace('|', "\\|");
            line.push(text);
            if r == 0 {
                align.push(cell_align(dom, cell));
            }

            let mut span = dom
                .attr(cell, "colspan")
                .and_then(|s| s.trim().parse::<i32>().ok())
                .unwrap_or(1);
            if span < 1 {
                span = 1;
            }
            if span > 50 {
                span = 50;
            }
            for _ in 1..span {
                line.push(String::new());
                if r == 0 {
                    align.push(String::new());
                }
            }
        }
        cols = cols.max(line.len());
        grid.push(line);
    }
    if cols == 0 {
        return String::new();
    }
    while align.len() < cols {
        align.push(String::new());
    }

    let mut out = String::new();
    let cap = one_line(&caption, " ");
    if !cap.is_empty() {
        out.push_str(&cap);
        out.push_str("\n\n");
    }

    for r in 0..grid.len() {
        while grid[r].len() < cols {
            grid[r].push(String::new());
        }
        out.push_str("| ");
        out.push_str(&grid[r].join(" | "));
        out.push_str(" |\n");
        if r == 0 {
            out.push('|');
            for k in 0..cols {
                out.push_str(match align[k].as_str() {
                    "center" => " :---: |",
                    "right" => " ---: |",
                    "left" => " :--- |",
                    _ => " --- |",
                });
            }
            out.push('\n');
        }
    }
    out.trim_end_matches('\n').to_string()
}

fn collect_rows(dom: &Dom, n: usize, rows: &mut Vec<Vec<usize>>, caption: &mut String) {
    for &c in &dom.nodes[n].children {
        let name = match dom.name(c) {
            Some(nm) => nm,
            None => continue,
        };
        if skip(dom, c) {
            continue;
        }
        match name {
            "tr" => {
                let cells: Vec<usize> = dom.nodes[c]
                    .children
                    .iter()
                    .copied()
                    .filter(|&d| matches!(dom.name(d), Some("td") | Some("th")))
                    .collect();
                if !cells.is_empty() {
                    rows.push(cells);
                }
            }
            "thead" | "tbody" | "tfoot" => collect_rows(dom, c, rows, caption),
            "caption" => {
                for &d in &dom.nodes[c].children {
                    render_inline(dom, d, caption);
                }
            }
            _ => {}
        }
    }
}

fn cell_align(dom: &Dom, cell: usize) -> String {
    let mut a: Option<String> = dom.attr(cell, "align").map(|s| s.to_string());
    if a.is_none() {
        if let Some(style) = dom.attr(cell, "style") {
            let lower = style.to_ascii_lowercase();
            if let Some(k) = lower.find("text-align") {
                let mut rest = &lower[k..];
                if let Some(semi) = rest.find(';') {
                    rest = &rest[..semi];
                }
                a = if rest.contains("center") {
                    Some("center".to_string())
                } else if rest.contains("right") {
                    Some("right".to_string())
                } else if rest.contains("left") {
                    Some("left".to_string())
                } else {
                    None
                };
            }
        }
    }
    match a {
        Some(s) => s.trim().to_lowercase(),
        None => String::new(),
    }
}

// ------------------------------------------------------------------
// Step 3: inline output
// ------------------------------------------------------------------

/// Gets the plain text under a node (used for code and for comparing link text).
fn raw_text(dom: &Dom, id: usize, sb: &mut String) {
    let name = match dom.name(id) {
        None => {
            sb.push_str(&decode(&dom.nodes[id].text));
            return;
        }
        Some(n) => n,
    };
    if skip(dom, id) {
        return;
    }
    if name == "br" {
        sb.push('\n');
        return;
    }
    for &c in &dom.nodes[id].children {
        raw_text(dom, c, sb);
    }
}

fn render_inline(dom: &Dom, id: usize, sb: &mut String) {
    let name = match dom.name(id) {
        None => {
            let mut t = decode(&collapse_ws(&dom.nodes[id].text));
            if t.starts_with(' ') && last_char(sb).map_or(false, is_space) {
                t.remove(0);
            }
            if !t.is_empty() {
                sb.push_str(&escape(&t));
            }
            return;
        }
        Some(n) => n,
    };
    if skip(dom, id) {
        return;
    }

    match name {
        "br" => sb.push(BR),
        "wbr" | "input" => {}

        "strong" | "b" => wrap(dom, id, sb, "**", "**"),
        "em" | "i" | "cite" => wrap(dom, id, sb, "*", "*"),
        "del" | "s" | "strike" => wrap(dom, id, sb, "~~", "~~"),

        "kbd" | "sub" | "sup" | "ins" => {
            let open = format!("<{}>", name);
            let close = format!("</{}>", name);
            wrap(dom, id, sb, &open, &close);
        }
        "u" => wrap(dom, id, sb, "<ins>", "</ins>"),

        "code" | "tt" | "samp" => {
            let mut raw = String::new();
            raw_text(dom, id, &mut raw);
            let code = collapse_ws(&raw).trim().to_string();
            if !code.is_empty() {
                put(sb, &code_span(&code));
            }
        }

        "a" => link(dom, id, sb),

        "img" => {
            let src = dom.attr(id, "src");
            let alt = collapse_ws(dom.attr(id, "alt").unwrap_or("")).trim().to_string();
            match src {
                Some(s) if !s.trim().is_empty() => {
                    let alt = alt.replace('\\', "\\\\").replace('[', "\\[").replace(']', "\\]");
                    sb.push_str("![");
                    sb.push_str(&alt);
                    sb.push_str("](");
                    sb.push_str(&dest(s));
                    sb.push_str(&title_part(dom.attr(id, "title")));
                    sb.push(')');
                }
                _ => {
                    if !alt.is_empty() {
                        put(sb, &escape(&alt));
                    }
                }
            }
        }

        _ => {
            let block = is_block(name);
            if block {
                sb.push(BR);
            }
            for &c in &dom.nodes[id].children {
                render_inline(dom, c, sb);
            }
            if block {
                sb.push(BR);
            }
        }
    }
}

fn wrap(dom: &Dom, id: usize, sb: &mut String, open: &str, close: &str) {
    let mut inner = String::new();
    for &c in &dom.nodes[id].children {
        render_inline(dom, c, &mut inner);
    }
    let (lead, core, trail) = edge(&inner);
    put(sb, lead);
    if core.is_empty() {
        return;
    }
    sb.push_str(open);
    sb.push_str(&core);
    sb.push_str(close);
    if !trail.is_empty() {
        sb.push(' ');
    }
}

fn link(dom: &Dom, id: usize, sb: &mut String) {
    let href = dom.attr(id, "href").unwrap_or("").trim().to_string();
    let mut inner = String::new();
    for &c in &dom.nodes[id].children {
        render_inline(dom, c, &mut inner);
    }

    if href.is_empty() || starts_with_ci(&href, "javascript:") {
        put(sb, &inner);
        return;
    }

    let mut raw = String::new();
    raw_text(dom, id, &mut raw);
    let plain = collapse_ws(&raw).trim().to_string();
    let absolute = starts_with_ci(&href, "http://")
        || starts_with_ci(&href, "https://")
        || starts_with_ci(&href, "mailto:");
    if absolute && plain == href && !href.contains(|ch| ch == ' ' || ch == '<' || ch == '>') {
        put(sb, &format!("<{}>", href));
        return;
    }

    let (lead, core, trail) = edge(&inner);
    let core = core.replace(BR, " ");
    if core.is_empty() {
        return;
    }

    put(sb, lead);
    sb.push('[');
    sb.push_str(&core);
    sb.push_str("](");
    sb.push_str(&dest(&href));
    sb.push_str(&title_part(dom.attr(id, "title")));
    sb.push(')');
    if !trail.is_empty() {
        sb.push(' ');
    }
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_strip_scripts_and_styles() {
        let html = "<html><head><script>alert(1);</script><style>body { color: red; }</style></head><body><h1>Title</h1><p>Hello &amp; welcome</p></body></html>";
        let md = html_to_markdown(html);
        assert!(!md.contains("alert"));
        assert!(!md.contains("color: red"));
        assert!(md.contains("# Title"));
        assert!(md.contains("Hello & welcome"));
    }

    #[test]
    fn test_html_formatting() {
        let html = "<p>Text with <b>bold</b> and <i>italic</i> and <code>code</code></p><ul><li>One</li><li>Two</li></ul>";
        let md = html_to_markdown(html);
        assert!(md.contains("**bold**"));
        assert!(md.contains("*italic*"));
        assert!(md.contains("`code`"));
        assert!(md.contains("- One"));
        assert!(md.contains("- Two"));
    }

    #[test]
    fn test_utf8_non_ascii() {
        let html = "<p>داستان <b>رستم و سهراب</b> در شاهنامه فردوسی</p>";
        let md = html_to_markdown(html);
        assert!(md.contains("**رستم و سهراب**"));
    }

    #[test]
    fn test_links_images_and_autolinks() {
        let html = r#"<p>See <a href="https://a.com/x" title="T">the site</a>, <a href="https://b.com">https://b.com</a> <img src="p.png" alt="pic"></p>"#;
        let md = html_to_markdown(html);
        assert!(md.contains("[the site](https://a.com/x \"T\")"));
        assert!(md.contains("<https://b.com>"));
        assert!(md.contains("![pic](p.png)"));
    }

    #[test]
    fn test_nested_list_and_tasks() {
        let html = "<ul><li>A<ul><li>B</li></ul></li><li><input type=checkbox checked> done</li></ul>";
        let md = html_to_markdown(html);
        assert_eq!(md, "- A\n  - B\n- [x] done\n");
    }

    #[test]
    fn test_table_with_alignment() {
        let html = r#"<table><tr><th align="right">N</th><th>M</th></tr><tr><td>1</td><td>a|b</td></tr></table>"#;
        let md = html_to_markdown(html);
        assert_eq!(md, "| N | M |\n| ---: | --- |\n| 1 | a\\|b |\n");
    }

    #[test]
    fn test_code_block_and_escape() {
        let html = "<pre><code class=\"language-rust\">let a = 1;\n</code></pre><p>1. not a list *x*</p>";
        let md = html_to_markdown(html);
        assert!(md.contains("```rust\nlet a = 1;\n```"));
        assert!(md.contains("1\\. not a list \\*x\\*"));
    }

    #[test]
    fn test_entities() {
        assert_eq!(decode_html_entities("a &lt; b &#65; &#x42; &bogus;"), "a < b A B &bogus;");
    }
}