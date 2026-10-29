#[derive(Clone, Copy)]
pub enum Style {
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
    Text,
    Bullet,
    Ordered(u32),
    Task(bool),
    Quote,
    Code,
    Rule,
    TableRow,
    TableSep,
    Image,
    Math,
    Diagram,
    Footnote,
}

#[derive(Clone)]
pub struct Link {
    pub label: String,
    pub target: String,
    pub title: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone)]
pub struct TableData {
    pub cells: Vec<String>,
    pub align: Vec<TableAlign>,
}

#[derive(Clone)]
pub struct CodeBlock {
    pub language: String,
    pub text: String,
}

#[derive(Clone)]
pub struct Line {
    pub style: Style,
    pub markup: String,
    pub anchor: Option<String>,
    pub indent: u8,
    pub image: Option<Link>,
    pub code: Option<CodeBlock>,
    pub diagram: Option<crate::graph::Graph>,
    pub tooltip: Option<String>,
    pub table: Option<TableData>,
    pub extra_anchors: Vec<String>,
}

#[derive(Clone, Default)]
pub struct Document {
    pub lines: Vec<Line>,
    pub links: Vec<Link>,
    pub index: BlockIndex,
}

fn plain_line(style: Style, markup: String) -> Line {
    Line {
        style,
        markup,
        anchor: None,
        indent: 0,
        image: None,
        code: None,
        diagram: None,
        tooltip: None,
        table: None,
        extra_anchors: Vec::new(),
    }
}

type ReferenceMap = std::collections::HashMap<String, (String, Option<String>)>;

fn normalize_reference_label(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn parse_reference_definition(line: &str) -> Option<(String, String, Option<String>)> {
    let t = line.trim();
    if !t.starts_with('[') || t.starts_with("[^") {
        return None;
    }
    let rb = t.find("]:")?;
    let id = t[1..rb].trim();
    if id.is_empty() {
        return None;
    }
    let rest = t[rb + 2..].trim_start();
    let (target, title) = inline::markdown_destination(rest)?;
    Some((normalize_reference_label(id), target, title))
}

fn collect_references(src: &str) -> ReferenceMap {
    let mut refs = ReferenceMap::new();
    let mut fence: Option<(char, usize)> = None;
    for raw in src.lines() {
        let line = raw.trim_end_matches('\r');
        if let Some((fc, flen)) = fence {
            if is_fence_close(line, fc, flen) {
                fence = None;
            }
            continue;
        }
        if let Some((fc, flen, _)) = parse_fence_open(line) {
            fence = Some((fc, flen));
            continue;
        }
        if let Some((id, target, title)) = parse_reference_definition(line) {
            refs.insert(id, (target, title));
        }
    }
    refs
}

pub fn parse(src: &str) -> Document {
    let normalized = expand_setext_headings(src);
    let src = normalized.as_str();
    let mut doc = Document::default();
    let (footnotes, footnote_source_lines) = collect_footnotes(src);
    let references = collect_references(src);
    let mut code = false;
    let mut code_lang = String::new();
    let mut code_buf = String::new();
    let mut code_fence_char = '`';
    let mut code_fence_len = 3usize;
    let mut code_indent = 0u8;
    let mut code_fence_spaces = 0usize;
    let mut math_block = false;
    let mut math_fence = false;
    let mut math_fence_char = '`';
    let mut math_fence_len = 3usize;
    let mut math_indent = 0u8;
    let mut math_fence_spaces = 0usize;
    let mut math_buf = String::new();
    let mut anchors = std::collections::HashMap::<String, usize>::new();
    let mut footnote_ref_counts = std::collections::HashMap::<String, usize>::new();

    let source_lines: Vec<&str> = src.lines().collect();
    for (source_line_no, raw) in source_lines.iter().copied().enumerate() {
        if footnote_source_lines.contains(&source_line_no) {
            continue;
        }
        let line = raw.trim_end_matches('\r');
        let trimmed_all = line.trim();

        if !code && !math_block {
            if let Some((fc, flen, info)) = parse_fence_open(line) {
                if info.eq_ignore_ascii_case("math") {
                    math_fence = true;
                    math_fence_char = fc;
                    math_fence_len = flen;
                    let leading = line.len() - line.trim_start_matches(' ').len();
                    math_fence_spaces = leading.min(3);
                    math_indent = (leading / 2).min(8) as u8;
                    math_buf.clear();
                    continue;
                }
            }
        }
        if math_fence {
            if is_fence_close(line, math_fence_char, math_fence_len) {
                let mut ml = plain_line(Style::Math, crate::math::render(math_buf.trim()));
                ml.indent = math_indent;
                doc.lines.push(ml);
                math_buf.clear();
                math_fence = false;
            } else {
                if !math_buf.is_empty() {
                    math_buf.push('\n');
                }
                math_buf.push_str(strip_fence_content_indent(line, math_fence_spaces));
            }
            continue;
        }

        if !code && trimmed_all.starts_with("$$") {
            if math_block {
                if let Some(before) = trimmed_all.strip_suffix("$$") {
                    if !before.is_empty() {
                        if !math_buf.is_empty() {
                            math_buf.push(' ');
                        }
                        math_buf.push_str(before);
                    }
                    doc.lines.push(plain_line(Style::Math, crate::math::render(math_buf.trim())));
                    math_buf.clear();
                    math_block = false;
                } else {
                    if !math_buf.is_empty() {
                        math_buf.push(' ');
                    }
                    math_buf.push_str(trimmed_all);
                }
            } else {
                let rest = &trimmed_all[2..];
                if let Some(end) = rest.rfind("$$") {
                    doc.lines.push(plain_line(Style::Math, crate::math::render(rest[..end].trim())));
                } else {
                    math_block = true;
                    math_buf.push_str(rest.trim());
                }
            }
            continue;
        }
        if math_block {
            if !math_buf.is_empty() {
                math_buf.push(' ');
            }
            math_buf.push_str(trimmed_all);
            continue;
        }

        if code {
            if is_fence_close(line, code_fence_char, code_fence_len) {
                let text = code_buf.trim_end_matches('\n').to_string();
                let markup = escape_markup(&text);
                let diagram = match code_lang.to_ascii_lowercase().as_str() {
                    "mermaid" | "mmd" => crate::mermaid::parse(&text).ok(),
                    "dot" | "graphviz" | "gv" => crate::graphviz::parse(&text).ok(),
                    _ => None,
                };
                let style = if diagram.is_some() { Style::Diagram } else { Style::Code };
                let block = CodeBlock { language: code_lang.clone(), text };
                doc.lines.push(Line {
                    style,
                    markup,
                    anchor: None,
                    indent: code_indent,
                    image: None,
                    code: Some(block),
                    diagram,
                    tooltip: None,
                    table: None,
                    extra_anchors: Vec::new(),
                });
                code = false;
                code_lang.clear();
                code_buf.clear();
            } else {
                code_buf.push_str(strip_fence_content_indent(line, code_fence_spaces));
                code_buf.push('\n');
            }
            continue;
        }
        if let Some((fc, flen, info)) = parse_fence_open(line) {
            code = true;
            code_fence_char = fc;
            code_fence_len = flen;
            let leading = line.len() - line.trim_start_matches(' ').len();
            code_fence_spaces = leading.min(3);
            code_indent = (leading / 2).min(8) as u8;
            code_lang = info.to_string();
            code_buf.clear();
            continue;
        }

        if parse_footnote_definition(line).is_some() || parse_reference_definition(line).is_some() {
            continue;
        }

        let leading = line.len() - line.trim_start_matches(' ').len();
        let base_indent = (leading / 2).min(8) as u8;
        let trimmed = line.trim_start_matches(' ');
        // Parse blockquote markers as a container first. This lets constructs such as
        // `> - item` and `> 1. item` keep their inner list semantics instead of
        // degrading to plain quote text.
        let (quote_depth, content) = if trimmed.starts_with('>') { parse_quote_prefix(trimmed) } else { (0, trimmed) };

        // CommonMark indented code block: four columns of leading indentation.
        // It cannot interrupt a paragraph; coalescing below merges consecutive
        // indented code lines into one block. HTML remains intentionally literal.
        if quote_depth == 0
            && leading_indent_columns(line) >= 4
            && !doc.lines.last().is_some_and(|prev| matches!(prev.style, Style::Text) && !prev.markup.is_empty())
            && !continues_list(doc.lines.last(), content)
        {
            let text = strip_four_indent_columns(line);
            let markup = escape_markup(&text);
            doc.lines.push(Line {
                style: Style::Code,
                markup,
                anchor: None,
                indent: 0,
                image: None,
                code: Some(CodeBlock { language: String::new(), text }),
                diagram: None,
                tooltip: None,
                table: None,
                extra_anchors: Vec::new(),
            });
            continue;
        }

        let image = parse_image_with_refs(content, &references);
        let (style, body) = if image.is_some() {
            (Style::Image, "")
        } else if let Some(x) = content.strip_prefix("###### ") {
            (Style::H6, x)
        } else if let Some(x) = content.strip_prefix("##### ") {
            (Style::H5, x)
        } else if let Some(x) = content.strip_prefix("#### ") {
            (Style::H4, x)
        } else if let Some(x) = content.strip_prefix("### ") {
            (Style::H3, x)
        } else if let Some(x) = content.strip_prefix("## ") {
            (Style::H2, x)
        } else if let Some(x) = content.strip_prefix("# ") {
            (Style::H1, x)
        } else if is_table_separator(content) {
            (Style::TableSep, "")
        } else if is_table_row(content) {
            (Style::TableRow, content)
        } else if let Some(x) =
            content.strip_prefix("- [ ] ").or_else(|| content.strip_prefix("* [ ] ")).or_else(|| content.strip_prefix("+ [ ] "))
        {
            (Style::Task(false), x)
        } else if let Some(x) = content
            .strip_prefix("- [x] ")
            .or_else(|| content.strip_prefix("- [X] "))
            .or_else(|| content.strip_prefix("* [x] "))
            .or_else(|| content.strip_prefix("* [X] "))
            .or_else(|| content.strip_prefix("+ [x] "))
            .or_else(|| content.strip_prefix("+ [X] "))
        {
            (Style::Task(true), x)
        } else if let Some((n, x)) = parse_ordered_item(content) {
            (Style::Ordered(n), x)
        } else if let Some(x) =
            content.strip_prefix("- ").or_else(|| content.strip_prefix("* ")).or_else(|| content.strip_prefix("+ "))
        {
            (Style::Bullet, x)
        } else if quote_depth > 0 {
            (Style::Quote, content)
        } else if is_thematic_break(content) {
            (Style::Rule, "")
        } else {
            (Style::Text, line)
        };

        let indent = base_indent.saturating_add(quote_depth.saturating_sub(1));

        let anchor = if matches!(style, Style::H1 | Style::H2 | Style::H3 | Style::H4 | Style::H5 | Style::H6) {
            let plain = strip_inline_markup(body);
            let base = slugify(&plain);
            if base.is_empty() {
                None
            } else {
                let n = anchors.entry(base.clone()).or_insert(0);
                let id = if *n == 0 { base } else { format!("{}_{}", base, *n) };
                *n += 1;
                Some(id)
            }
        } else {
            None
        };

        // CommonMark hard line break: an unescaped backslash at the end of a
        // paragraph line, followed by another line of the same paragraph.
        let backslash_break = matches!(style, Style::Text)
            && ends_with_hard_break_backslash(body)
            && source_lines.get(source_line_no + 1).is_some_and(|next| !next.trim().is_empty());
        let body = if backslash_break { &body[..body.len() - 1] } else { body };

        let (mut markup, links) = match style {
            Style::TableRow => parse_table_markup(body, &references),
            Style::TableSep | Style::Rule | Style::Image | Style::Math | Style::Diagram | Style::Code => {
                (String::new(), Vec::new())
            }
            _ => parse_inline_ctx(body, &references),
        };
        if let Some(id) = &anchor {
            markup.push_str("  <a href=\"#");
            markup.push_str(&percent_encode_fragment(id));
            markup.push_str("\">¶</a>");
        }
        if (backslash_break || matches!(style, Style::Text) && line.ends_with("  ")) && !markup.is_empty() {
            markup.push('\n');
        }
        // Count only footnote references that the inline parser actually
        // recognized. This avoids false return anchors for escaped markers or
        // `[^text]` appearing inside code spans.
        let mut extra_anchors = Vec::new();
        for link in links.iter().filter(|l| l.target.starts_with("#fn_")) {
            let id = link.label.clone();
            let key = normalize_reference_label(&id);
            let n = footnote_ref_counts.entry(key).or_insert(0);
            *n += 1;
            extra_anchors.push(format!("fnref_{}_{}", slugify(&id), *n));
        }
        let tooltip = links.iter().find_map(|l| l.title.clone());
        doc.links.extend(links);
        let table = match style {
            Style::TableRow => Some(TableData { cells: parse_table_cells(body), align: Vec::new() }),
            Style::TableSep => Some(TableData { cells: Vec::new(), align: parse_table_alignments(content) }),
            _ => None,
        };
        doc.lines.push(Line { style, markup, anchor, indent, image, code: None, diagram: None, tooltip, table, extra_anchors });
    }
    if code && !code_buf.is_empty() {
        let text = code_buf.trim_end_matches('\n').to_string();
        let markup = escape_markup(&text);
        let diagram = match code_lang.to_ascii_lowercase().as_str() {
            "mermaid" | "mmd" => crate::mermaid::parse(&text).ok(),
            "dot" | "graphviz" | "gv" => crate::graphviz::parse(&text).ok(),
            _ => None,
        };
        let style = if diagram.is_some() { Style::Diagram } else { Style::Code };
        doc.lines.push(Line {
            style,
            markup,
            anchor: None,
            indent: code_indent,
            image: None,
            code: Some(CodeBlock { language: code_lang, text }),
            diagram,
            tooltip: None,
            table: None,
            extra_anchors: Vec::new(),
        });
    }
    if (math_block || math_fence) && !math_buf.trim().is_empty() {
        let mut ml = plain_line(Style::Math, crate::math::render(math_buf.trim()));
        if math_fence {
            ml.indent = math_indent;
        }
        doc.lines.push(ml);
    }
    coalesce_code_lines(&mut doc.lines);
    coalesce_text_lines(&mut doc.lines);
    if !footnotes.is_empty() {
        doc.lines.push(plain_line(Style::Rule, String::new()));
        let mut h = plain_line(Style::H4, "Footnotes".into());
        h.anchor = Some("footnotes".into());
        doc.lines.push(h);
        for note in footnotes {
            let safe_id = slugify(&note.id);
            let ref_count = footnote_ref_counts.get(&normalize_reference_label(&note.id)).copied().unwrap_or(0);
            let mut first_block = true;
            let mut anchor_assigned = false;
            let mut in_fence: Option<(char, usize, String, String)> = None;
            for raw in note.blocks.iter() {
                let t = raw.trim();
                if let Some((fc, flen, lang, buf)) = in_fence.as_mut() {
                    if is_fence_close(raw, *fc, *flen) {
                        let text = buf.trim_end_matches('\n').to_string();
                        let mut markup = String::new();
                        if first_block {
                            markup.push_str(&format!("<sup>{}</sup>  ", escape_markup(&note.id)));
                            first_block = false;
                        }
                        if !lang.is_empty() {
                            markup.push_str(&format!("<small>{}</small>  ", escape_markup(lang)));
                        }
                        markup.push_str("<tt>");
                        markup.push_str(&escape_markup(&text));
                        markup.push_str("</tt>");
                        let mut line = plain_line(Style::Footnote, markup);
                        if !anchor_assigned {
                            line.anchor = Some(format!("fn_{}", safe_id));
                            anchor_assigned = true;
                        }
                        doc.lines.push(line);
                        in_fence = None;
                    } else {
                        buf.push_str(raw);
                        buf.push('\n');
                    }
                    continue;
                }
                if let Some((fc, flen, info)) = parse_fence_open(raw) {
                    in_fence = Some((fc, flen, info.to_string(), String::new()));
                    continue;
                }
                if t.is_empty() {
                    let mut line = plain_line(Style::Footnote, String::new());
                    if !anchor_assigned {
                        line.anchor = Some(format!("fn_{}", safe_id));
                        anchor_assigned = true;
                    }
                    doc.lines.push(line);
                    continue;
                }
                let (prefix, content) = if let Some((n, x)) = parse_ordered_item(t) {
                    (format!("{}.  ", n), x)
                } else if let Some(x) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ ")) {
                    ("•  ".to_string(), x)
                } else {
                    (String::new(), raw.as_str())
                };
                let (body, links) = parse_inline_ctx(content, &references);
                doc.links.extend(links);
                let mut markup = String::new();
                if first_block {
                    markup.push_str(&format!("<sup>{}</sup>  ", escape_markup(&note.id)));
                    first_block = false;
                }
                markup.push_str(&prefix);
                markup.push_str(&body);
                let mut line = plain_line(Style::Footnote, markup);
                if !anchor_assigned {
                    line.anchor = Some(format!("fn_{}", safe_id));
                    anchor_assigned = true;
                }
                doc.lines.push(line);
            }
            if let Some((_fc, _flen, lang, buf)) = in_fence.take() {
                let mut markup = String::new();
                if first_block {
                    markup.push_str(&format!("<sup>{}</sup>  ", escape_markup(&note.id)));
                }
                if !lang.is_empty() {
                    markup.push_str(&format!("<small>{}</small>  ", escape_markup(&lang)));
                }
                markup.push_str("<tt>");
                markup.push_str(&escape_markup(buf.trim_end_matches('\n')));
                markup.push_str("</tt>");
                let mut line = plain_line(Style::Footnote, markup);
                if !anchor_assigned {
                    line.anchor = Some(format!("fn_{}", safe_id));
                }
                doc.lines.push(line);
            }
            if ref_count > 0 {
                let mut backs = String::from("<small>Back: ");
                for n in 1..=ref_count {
                    if n > 1 {
                        backs.push_str(" ");
                    }
                    let target = format!("#fnref_{}_{}", safe_id, n);
                    backs.push_str(&format!("<a href=\"{}\">↩{}</a>", escape_attr(&target), n));
                    doc.links.push(Link {
                        label: format!("↩{}", n),
                        target,
                        title: Some(format!("Back to footnote reference {}", n)),
                    });
                }
                backs.push_str("</small>");
                doc.lines.push(plain_line(Style::Footnote, backs));
            }
        }
    }
    doc.index = BlockIndex::build(&doc.lines);
    doc
}

fn coalesce_code_lines(lines: &mut Vec<Line>) {
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    for mut line in lines.drain(..) {
        if matches!(line.style, Style::Code) && line.code.as_ref().is_some_and(|code| code.language.is_empty()) {
            if let Some(prev) = out.last_mut() {
                if matches!(prev.style, Style::Code) && prev.code.as_ref().is_some_and(|code| code.language.is_empty()) {
                    if let (Some(prev_code), Some(code)) = (prev.code.as_mut(), line.code.take()) {
                        prev_code.text.push('\n');
                        prev_code.text.push_str(&code.text);
                        prev.markup.push('\n');
                        prev.markup.push_str(&line.markup);
                        continue;
                    }
                }
            }
        }
        out.push(line);
    }
    *lines = out;
}

/// True when `s` ends with an odd number of backslashes, i.e. the last one is
/// not itself escaped and therefore marks a hard line break.
fn ends_with_hard_break_backslash(s: &str) -> bool {
    (s.len() - s.trim_end_matches('\\').len()) % 2 == 1
}

/// A deeply indented list marker directly after a list item is a nested item,
/// not an indented code block (`- a` / `  - b` / `    - c`).
fn continues_list(prev: Option<&Line>, content: &str) -> bool {
    prev.is_some_and(|p| matches!(p.style, Style::Bullet | Style::Ordered(_) | Style::Task(_)))
        && (content.starts_with("- ")
            || content.starts_with("* ")
            || content.starts_with("+ ")
            || parse_ordered_item(content).is_some())
}

fn leading_indent_columns(s: &str) -> usize {
    let mut col = 0usize;
    for ch in s.chars() {
        match ch {
            ' ' => col += 1,
            '\t' => col += 4 - (col % 4),
            _ => break,
        }
    }
    col
}

fn strip_four_indent_columns(s: &str) -> String {
    let mut col = 0usize;
    let mut byte = 0usize;
    for (i, ch) in s.char_indices() {
        if col >= 4 {
            byte = i;
            break;
        }
        match ch {
            ' ' => col += 1,
            '\t' => col += 4 - (col % 4),
            _ => {
                byte = i;
                break;
            }
        }
        byte = i + ch.len_utf8();
    }
    let mut out = String::new();
    if col > 4 {
        out.push_str(&" ".repeat(col - 4));
    }
    out.push_str(&s[byte..]);
    out
}

fn coalesce_text_lines(lines: &mut Vec<Line>) {
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    for line in lines.drain(..) {
        if matches!(line.style, Style::Text) && !line.markup.is_empty() {
            if let Some(prev) = out.last_mut() {
                if matches!(prev.style, Style::Text) && !prev.markup.is_empty() {
                    if !prev.markup.ends_with('\n') {
                        prev.markup.push(' ');
                    }
                    prev.markup.push_str(&line.markup);
                    if prev.tooltip.is_none() {
                        prev.tooltip = line.tooltip;
                    }
                    prev.extra_anchors.extend(line.extra_anchors);
                    continue;
                }
                // CommonMark lazy continuation: a non-blank paragraph line directly
                // following a list item belongs to that item until a block boundary.
                if matches!(prev.style, Style::Bullet | Style::Ordered(_) | Style::Task(_) | Style::Quote)
                    && line.indent <= prev.indent.saturating_add(1)
                {
                    if !prev.markup.ends_with('\n') {
                        prev.markup.push(' ');
                    }
                    prev.markup.push_str(&line.markup);
                    if prev.tooltip.is_none() {
                        prev.tooltip = line.tooltip;
                    }
                    prev.extra_anchors.extend(line.extra_anchors);
                    continue;
                }
            }
        }
        out.push(line);
    }
    *lines = out;
}

fn strip_fence_content_indent(s: &str, max_spaces: usize) -> &str {
    let mut remove = 0usize;
    for b in s.as_bytes().iter().take(max_spaces) {
        if *b == b' ' {
            remove += 1;
        } else {
            break;
        }
    }
    &s[remove..]
}

fn fence_indent_and_tail(s: &str) -> Option<(usize, &str)> {
    let mut cols = 0usize;
    let mut byte = 0usize;
    for (i, ch) in s.char_indices() {
        match ch {
            ' ' if cols < 4 => {
                cols += 1;
                byte = i + 1;
            }
            '\t' => return None,
            _ => {
                byte = i;
                break;
            }
        }
    }
    if cols > 3 { None } else { Some((cols, &s[byte..])) }
}

fn parse_fence_open(s: &str) -> Option<(char, usize, &str)> {
    let (_, t) = fence_indent_and_tail(s)?;
    let fc = t.chars().next()?;
    if fc != '`' && fc != '~' {
        return None;
    }
    let len = t.chars().take_while(|&c| c == fc).count();
    if len < 3 {
        return None;
    }
    let info = t[len..].trim(); // fence characters are ASCII bytes
    if fc == '`' && info.contains('`') {
        return None;
    }
    Some((fc, len, info))
}

fn is_fence_close(s: &str, fc: char, min_len: usize) -> bool {
    let Some((_, t)) = fence_indent_and_tail(s) else {
        return false;
    };
    let t = t.trim_end();
    let len = t.chars().take_while(|&c| c == fc).count();
    len >= min_len && len == t.chars().count()
}

fn expand_setext_headings(src: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = String::with_capacity(src.len() + 32);
    let mut i = 0usize;
    let mut fence: Option<(char, usize)> = None;
    while i < lines.len() {
        let line = lines[i].trim_end_matches('\r');
        if let Some((fc, flen)) = fence {
            out.push_str(line);
            out.push('\n');
            if is_fence_close(line, fc, flen) {
                fence = None;
            }
            i += 1;
            continue;
        }
        if let Some((fc, flen, _)) = parse_fence_open(line) {
            fence = Some((fc, flen));
            out.push_str(line);
            out.push('\n');
            i += 1;
            continue;
        }
        let t = line.trim();
        if i + 1 < lines.len() && !t.is_empty() && !t.starts_with('#') {
            let u = lines[i + 1].trim();
            let eq = !u.is_empty() && u.chars().all(|c| c == '=');
            let dash = !u.is_empty() && u.chars().all(|c| c == '-');
            if eq || dash {
                out.push_str(if eq { "# " } else { "## " });
                out.push_str(t);
                out.push('\n');
                i += 2;
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
        i += 1;
    }
    out
}

fn is_thematic_break(s: &str) -> bool {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() < 3 {
        return false;
    }
    let mut it = compact.chars();
    let Some(first) = it.next() else {
        return false;
    };
    matches!(first, '-' | '*' | '_') && it.all(|c| c == first)
}

fn parse_quote_prefix(s: &str) -> (u8, &str) {
    let mut rest = s;
    let mut depth = 0u8;
    loop {
        let t = rest.trim_start_matches(' ');
        if let Some(x) = t.strip_prefix('>') {
            depth = depth.saturating_add(1);
            rest = x.strip_prefix(' ').unwrap_or(x);
        } else {
            break;
        }
    }
    (depth.max(1), rest)
}

fn parse_footnote_definition(s: &str) -> Option<(String, String)> {
    let t = s.trim_start();
    let rest = t.strip_prefix("[^")?;
    let rb = rest.find("]:")?;
    let id = rest[..rb].trim();
    if id.is_empty() {
        return None;
    }
    Some((id.to_string(), rest[rb + 2..].trim_start().to_string()))
}

#[derive(Clone, Debug)]
struct FootnoteDef {
    id: String,
    blocks: Vec<String>,
}

/// Collect GFM-style footnote definitions, including indented continuation
/// lines. Continuations use four spaces or one tab; blank lines are retained
/// when followed by another indented continuation. The returned index set is
/// used to remove definition source lines from the main document stream.
fn collect_footnotes(src: &str) -> (Vec<FootnoteDef>, std::collections::HashSet<usize>) {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    let mut consumed = std::collections::HashSet::new();
    let mut fence: Option<(char, usize)> = None;
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i].trim_end_matches('\r');
        if let Some((fc, flen)) = fence {
            if is_fence_close(line, fc, flen) {
                fence = None;
            }
            i += 1;
            continue;
        }
        if let Some((fc, flen, _)) = parse_fence_open(line) {
            fence = Some((fc, flen));
            i += 1;
            continue;
        }
        let Some((id, first)) = parse_footnote_definition(line) else {
            i += 1;
            continue;
        };
        consumed.insert(i);
        let mut blocks = vec![first];
        let mut j = i + 1;
        let mut pending_blank = false;
        while j < lines.len() {
            let raw = lines[j].trim_end_matches('\r');
            if raw.trim().is_empty() {
                pending_blank = true;
                j += 1;
                continue;
            }
            let cont = if let Some(x) = raw.strip_prefix("    ") {
                Some(x)
            } else if let Some(x) = raw.strip_prefix('\t') {
                Some(x)
            } else {
                None
            };
            let Some(body) = cont else {
                break;
            };
            if pending_blank {
                blocks.push(String::new());
                pending_blank = false;
            }
            blocks.push(body.to_string());
            consumed.insert(j);
            j += 1;
        }
        // Blank lines between the footnote and its continuation are part of
        // the definition only when a continuation actually followed them.
        if j > i + 1 {
            let mut k = i + 1;
            while k < j {
                if lines[k].trim().is_empty() {
                    consumed.insert(k);
                }
                k += 1;
            }
        }
        out.push(FootnoteDef { id, blocks });
        i = j.max(i + 1);
    }
    (out, consumed)
}

fn parse_ordered_item(s: &str) -> Option<(u32, &str)> {
    let marker = s.char_indices().find(|(_, ch)| *ch == '.' || *ch == ')').map(|(i, ch)| (i, ch))?;
    let marker_pos = marker.0;
    if marker_pos == 0 || marker_pos > 9 || !s[..marker_pos].bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let after_marker = marker_pos + marker.1.len_utf8();
    let rest = s.get(after_marker..)?;
    let mut consumed = 0usize;
    let mut columns = 0usize;
    for (i, ch) in rest.char_indices() {
        match ch {
            ' ' => columns += 1,
            '\t' => columns += 4 - (columns % 4),
            _ => break,
        }
        consumed = i + ch.len_utf8();
        if columns >= 4 {
            break;
        }
    }
    if columns == 0 || columns > 4 {
        return None;
    }
    let body = rest.get(consumed..)?;
    let n = s[..marker_pos].parse().ok()?;
    Some((n, body))
}

mod blocks;
mod inline;
pub use blocks::{Block, BlockIndex, count_hits};
use inline::{
    escape_attr, escape_markup, is_table_row, is_table_separator, parse_image_with_refs, parse_inline_ctx,
    parse_table_alignments, parse_table_cells, parse_table_markup,
};
pub use inline::{parse_inline_public, strip_inline_markup};

/// Stable section id used by shareable URLs.
/// Whitespace and punctuation runs become one underscore so the URL end is obvious
/// when pasted into plain-text mail or chat. Unicode letters/digits are preserved and
/// percent-encoded when needed.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut sep = false;
    for ch in s.trim().chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_alphanumeric() {
            if sep && !out.is_empty() {
                out.push('_');
            }
            sep = false;
            out.push(ch);
        } else {
            sep = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// Accept old v0.9 hyphen anchors and new underscore anchors as equivalent.
fn normalize_fragment_id(s: &str) -> String {
    slugify(&s.replace('-', " ").replace('_', " "))
}

pub fn percent_encode_fragment(s: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 15) as usize] as char);
        }
    }
    out
}

pub fn percent_decode_fragment(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(a), Some(b)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((a << 4) | b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The URL as shown in the address bar: spaces and non-ASCII characters are decoded so
/// `Patrick%20Olivelle%20%E2%80%94` reads as text. Every other escape stays encoded,
/// because decoding `%2F` or `%3F` would change what the URL means.
pub fn display_url(url: &str) -> String {
    let bytes = url.as_bytes();
    let mut out = String::with_capacity(url.len());
    let mut i = 0;
    while i < bytes.len() {
        let mut run = Vec::new();
        let mut j = i;
        while j + 2 < bytes.len() && bytes[j] == b'%' {
            match (hex(bytes[j + 1]), hex(bytes[j + 2])) {
                (Some(a), Some(b)) if ((a << 4) | b) == 0x20 || ((a << 4) | b) >= 0x80 => {
                    run.push((a << 4) | b);
                    j += 3;
                }
                _ => break,
            }
        }
        match std::str::from_utf8(&run) {
            Ok(text) if j > i => {
                out.push_str(text);
                i = j;
            }
            _ => {
                let ch = url[i..].chars().next().unwrap_or('%');
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    out
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn document_url(url: &str) -> String {
    url.split_once('#').map(|x| x.0).unwrap_or(url).to_string()
}

pub fn fragment_id(url: &str) -> Option<String> {
    let (_, raw) = url.split_once('#')?;
    let decoded = percent_decode_fragment(raw);
    let id = normalize_fragment_id(&decoded);
    if id.is_empty() { None } else { Some(id) }
}

pub fn canonicalize_url_fragment(url: &str) -> String {
    let base = document_url(url);
    match fragment_id(url) {
        Some(id) => format!("{}#{}", base, percent_encode_fragment(&id)),
        None => base,
    }
}

pub fn with_fragment_from(base: &str, source: &str) -> String {
    match fragment_id(source) {
        Some(id) => format!("{}#{}", document_url(base), percent_encode_fragment(&id)),
        None => document_url(base),
    }
}

#[cfg(test)]
mod tests;
