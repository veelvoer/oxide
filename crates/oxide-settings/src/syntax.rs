//! A small syntax highlighter for the languages Oxide's config files use: QML and JavaScript, kitty.conf, fish and
//! other shells, TOML, JSON, CSS, INI, Lua, Python and Markdown. It colors comments, strings, numbers, keywords and
//! keys with the theme's colors; it does not parse, so it never gets in the way of editing.
use std::path::Path;

use eframe::egui::{
    text::{LayoutJob, TextFormat},
    Color32, FontId, TextStyle, Ui,
};

use crate::theme::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Qml,
    Js,
    Kitty,
    Fish,
    Shell,
    Toml,
    Json,
    Css,
    Ini,
    Lua,
    Python,
    Markdown,
    Plain,
}

impl Lang {
    pub fn name(self) -> &'static str {
        match self {
            Lang::Qml => "QML",
            Lang::Js => "JavaScript",
            Lang::Kitty => "kitty config",
            Lang::Fish => "fish",
            Lang::Shell => "shell",
            Lang::Toml => "TOML",
            Lang::Json => "JSON",
            Lang::Css => "CSS",
            Lang::Ini => "INI",
            Lang::Lua => "Lua",
            Lang::Python => "Python",
            Lang::Markdown => "Markdown",
            Lang::Plain => "text",
        }
    }

    pub fn of(path: &Path) -> Lang {
        let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        match (name.as_str(), ext.as_str()) {
            ("kitty.conf", _) => Lang::Kitty,
            ("qmldir", _) => Lang::Plain,
            (_, "qml") => Lang::Qml,
            (_, "js" | "mjs") => Lang::Js,
            (_, "fish") => Lang::Fish,
            (_, "sh" | "bash" | "zsh") | ("bashrc" | ".bashrc" | ".zshrc", _) => Lang::Shell,
            (_, "toml") => Lang::Toml,
            (_, "json" | "jsonc") => Lang::Json,
            (_, "css") => Lang::Css,
            (_, "ini" | "desktop") => Lang::Ini,
            (_, "conf") => Lang::Kitty,
            (_, "lua") => Lang::Lua,
            (_, "py") => Lang::Python,
            (_, "md") => Lang::Markdown,
            _ => Lang::Plain,
        }
    }

    fn line_comment(self) -> &'static [&'static str] {
        match self {
            Lang::Qml | Lang::Js => &["//"],
            Lang::Kitty | Lang::Fish | Lang::Shell | Lang::Toml | Lang::Python => &["#"],
            Lang::Ini => &["#", ";"],
            Lang::Lua => &["--"],
            _ => &[],
        }
    }

    fn keywords(self) -> &'static [&'static str] {
        match self {
            Lang::Qml => &[
                "import", "as", "property", "signal", "function", "var", "let", "const", "if", "else", "for", "while", "do", "return", "switch", "case",
                "break", "continue", "new", "readonly", "required", "default", "alias", "component", "pragma", "on", "true", "false", "null", "undefined",
                "this", "typeof", "in", "of", "try", "catch", "throw", "enum",
            ],
            Lang::Js => &[
                "import", "export", "from", "function", "var", "let", "const", "if", "else", "for", "while", "do", "return", "switch", "case", "break",
                "continue", "new", "class", "extends", "true", "false", "null", "undefined", "this", "typeof", "in", "of", "try", "catch", "throw", "async", "await",
            ],
            Lang::Fish => &[
                "if", "else", "end", "for", "in", "while", "function", "set", "switch", "case", "begin", "return", "and", "or", "not", "break", "continue",
                "abbr", "alias", "source", "echo", "test", "status", "string", "math", "contains", "count", "set_color", "command", "builtin", "exit",
            ],
            Lang::Shell => &["if", "then", "else", "elif", "fi", "for", "in", "do", "done", "while", "case", "esac", "function", "return", "export", "local", "echo", "source", "alias", "exit"],
            Lang::Lua => &["local", "function", "end", "if", "then", "else", "elseif", "for", "while", "do", "return", "in", "and", "or", "not", "nil", "true", "false", "require"],
            Lang::Python => &["def", "class", "import", "from", "as", "if", "elif", "else", "for", "while", "return", "with", "try", "except", "finally", "in", "not", "and", "or", "True", "False", "None", "lambda", "pass", "raise", "yield"],
            Lang::Toml | Lang::Json => &["true", "false", "null"],
            Lang::Css => &["important"],
            _ => &[],
        }
    }
}

struct Palette {
    text: Color32,
    comment: Color32,
    string: Color32,
    number: Color32,
    keyword: Color32,
    key: Color32,
    punct: Color32,
}

fn palette() -> Palette {
    Palette { text: TEXT(), comment: MUTED(), string: GOOD(), number: WARN(), keyword: ACCENT(), key: BLUE(), punct: COPPER() }
}

fn push(job: &mut LayoutJob, s: &str, color: Color32, font: &FontId) {
    if s.is_empty() {
        return;
    }
    job.append(s, 0.0, TextFormat { font_id: font.clone(), color, ..Default::default() });
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' && false
}

/// Color one line. `in_block` tracks a /* */ comment that continues across lines.
fn line(job: &mut LayoutJob, text: &str, lang: Lang, p: &Palette, font: &FontId, in_block: &mut bool) {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let comments = lang.line_comment();
    let kw = lang.keywords();
    let block_capable = matches!(lang, Lang::Qml | Lang::Js | Lang::Css);
    let mut i = 0;
    let slice = |a: usize, b: usize| chars[a..b].iter().collect::<String>();

    // structural languages colored by position
    let trimmed_start = chars.iter().position(|c| !c.is_whitespace()).unwrap_or(n);
    match lang {
        Lang::Toml | Lang::Ini if trimmed_start < n && chars[trimmed_start] == '[' => {
            push(job, &slice(0, n), p.keyword, font);
            return;
        }
        Lang::Kitty if trimmed_start < n && !comments.iter().any(|c| chars[trimmed_start..].iter().collect::<String>().starts_with(c)) => {
            // `key value`: the first word is the key
            let mut e = trimmed_start;
            while e < n && !chars[e].is_whitespace() {
                e += 1;
            }
            push(job, &slice(0, trimmed_start), p.text, font);
            push(job, &slice(trimmed_start, e), p.key, font);
            i = e;
        }
        Lang::Toml | Lang::Ini => {
            if let Some(eq) = chars.iter().position(|c| *c == '=') {
                if chars[..eq].iter().all(|c| c.is_alphanumeric() || c.is_whitespace() || matches!(c, '_' | '-' | '.' | '"')) && !comments.iter().any(|c| text.trim_start().starts_with(c)) {
                    push(job, &slice(0, eq), p.key, font);
                    push(job, "=", p.punct, font);
                    i = eq + 1;
                }
            }
        }
        _ => {}
    }

    while i < n {
        let c = chars[i];
        if *in_block {
            let start = i;
            while i + 1 < n && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < n {
                i += 2;
                *in_block = false;
            } else {
                i = n;
            }
            push(job, &slice(start, i), p.comment, font);
            continue;
        }
        let rest: String = chars[i..n.min(i + 3)].iter().collect();
        if block_capable && rest.starts_with("/*") {
            *in_block = true;
            push(job, "/*", p.comment, font);
            i += 2;
            continue;
        }
        if comments.iter().any(|m| {
            rest.starts_with(m) && !(*m == "#" && lang == Lang::Css) && !(*m == "#" && matches!(lang, Lang::Shell | Lang::Fish) && i > 0 && !chars[i - 1].is_whitespace())
        }) {
            push(job, &slice(i, n), p.comment, font);
            return;
        }
        if c == '"' || c == '\'' || (c == '`' && matches!(lang, Lang::Qml | Lang::Js)) {
            let q = c;
            let start = i;
            i += 1;
            while i < n && chars[i] != q {
                if chars[i] == '\\' && i + 1 < n {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(n);
            // JSON / TOML keys: a string followed by `:` or `=` is a key
            let after = chars[i..].iter().find(|c| !c.is_whitespace()).copied();
            let color = if matches!(lang, Lang::Json) && after == Some(':') { p.key } else { p.string };
            push(job, &slice(start, i), color, font);
            continue;
        }
        if c.is_ascii_digit() || (c == '#' && matches!(lang, Lang::Css | Lang::Kitty | Lang::Toml | Lang::Ini | Lang::Qml) && i + 1 < n && chars[i + 1].is_ascii_hexdigit()) {
            let start = i;
            i += 1;
            while i < n && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_') {
                i += 1;
            }
            push(job, &slice(start, i), p.number, font);
            continue;
        }
        if c.is_alphabetic() || c == '_' || (c == '$' && matches!(lang, Lang::Fish | Lang::Shell)) || (c == '@' && lang == Lang::Css) {
            let start = i;
            i += 1;
            while i < n && (is_word(chars[i]) || (chars[i] == '-' && matches!(lang, Lang::Css | Lang::Fish | Lang::Shell))) {
                i += 1;
            }
            let w = slice(start, i);
            let after = chars[i..].iter().find(|c| !c.is_whitespace()).copied();
            let color = if w.starts_with('$') {
                p.key
            } else if kw.contains(&w.as_str()) {
                p.keyword
            } else if matches!(lang, Lang::Qml) && w.chars().next().is_some_and(|f| f.is_uppercase()) {
                p.key
            } else if matches!(lang, Lang::Qml | Lang::Js | Lang::Python | Lang::Lua) && after == Some('(') {
                p.key
            } else if lang == Lang::Css && after == Some(':') {
                p.key
            } else if lang == Lang::Qml && after == Some(':') {
                p.key
            } else {
                p.text
            };
            push(job, &w, color, font);
            continue;
        }
        let start = i;
        i += 1;
        let color = if "{}[]()<>=+-*/%!&|:;,.?".contains(c) { p.punct } else { p.text };
        push(job, &slice(start, i), color, font);
    }
}

/// The colored layout of `text` for the code editor.
pub fn layout_job(text: &str, lang: Lang, ui: &Ui, wrap_width: f32) -> LayoutJob {
    let font = TextStyle::Monospace.resolve(ui.style());
    let p = palette();
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    if lang == Lang::Plain {
        push(&mut job, text, p.text, &font);
        return job;
    }
    let mut in_block = false;
    let mut first = true;
    for l in text.split('\n') {
        if !first {
            push(&mut job, "\n", p.text, &font);
        }
        first = false;
        line(&mut job, l, lang, &p, &font, &mut in_block);
    }
    job
}
