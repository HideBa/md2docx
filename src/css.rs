use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct CssBorder {
    pub style: String,   // "solid" | "double" | "dotted" | "dashed" | "none"
    pub size_px: f32,
    pub color: String,   // "RRGGBB"
}

#[derive(Debug, Clone, Default)]
pub struct CssStyle {
    pub color: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub background_color: Option<String>,
    pub font_family: Option<String>,
    pub font_size_pt: Option<f32>,
    pub line_height: Option<f32>,
    pub border_top: Option<CssBorder>,
    pub border_bottom: Option<CssBorder>,
    pub border_left: Option<CssBorder>,
    pub border_right: Option<CssBorder>,
    pub padding_top_pt: Option<f32>,
    pub padding_right_pt: Option<f32>,
    pub padding_bottom_pt: Option<f32>,
    pub padding_left_pt: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct CssRules {
    pub h1: Option<CssStyle>,
    pub h2: Option<CssStyle>,
    pub h3: Option<CssStyle>,
    pub h4: Option<CssStyle>,
    pub h5: Option<CssStyle>,
    pub table_th: Option<CssStyle>,
    pub code_block: Option<CssStyle>,
    pub blockquote: Option<CssStyle>,
    pub classes: HashMap<String, CssStyle>,
}


pub fn load_css(path: &Path) -> Result<CssRules> {
    let input = std::fs::read_to_string(path)?;
    Ok(parse_css(&input))
}

pub fn parse_css(input: &str) -> CssRules {
    let stripped = strip_comments(input);
    let raw_rules = extract_rules(&stripped);
    let mut rules = CssRules::default();

    for (selector, body) in &raw_rules {
        let style = parse_declarations(body);
        match classify_selector(selector) {
            SelectorKind::H1 => rules.h1 = Some(style),
            SelectorKind::H2 => rules.h2 = Some(style),
            SelectorKind::H3 => rules.h3 = Some(style),
            SelectorKind::H4 => rules.h4 = Some(style),
            SelectorKind::H5 => rules.h5 = Some(style),
            SelectorKind::TableTh => rules.table_th = Some(style),
            SelectorKind::CodeBlock => rules.code_block = Some(style),
            SelectorKind::Blockquote => rules.blockquote = Some(style),
            SelectorKind::Class(name) => {
                rules.classes.insert(name, style);
            }
            SelectorKind::Unsupported => {
                eprintln!("warning: Unsupported selector `{}`", selector.trim());
            }
        }
    }

    rules
}

enum SelectorKind {
    H1,
    H2,
    H3,
    H4,
    H5,
    TableTh,
    CodeBlock,
    Blockquote,
    Class(String),
    Unsupported,
}

fn classify_selector(selector: &str) -> SelectorKind {
    let s = selector.trim();
    // カンマ区切りセレクタ対応: "pre, code" → CodeBlock
    if s.contains(',') {
        let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
        let all_code = parts.iter().all(|p| matches!(*p, "pre" | "code" | "pre code"));
        if all_code {
            return SelectorKind::CodeBlock;
        }
    }
    match s {
        "h1" => SelectorKind::H1,
        "h2" => SelectorKind::H2,
        "h3" => SelectorKind::H3,
        "h4" => SelectorKind::H4,
        "h5" => SelectorKind::H5,
        "table th" => SelectorKind::TableTh,
        "pre" | "code" | "pre code" => SelectorKind::CodeBlock,
        "blockquote" => SelectorKind::Blockquote,
        _ => {
            if let Some(class_name) = s.strip_prefix('.') {
                if !class_name.is_empty()
                    && !class_name.contains(' ')
                    && !class_name.contains('.')
                    && !class_name.contains('#')
                {
                    return SelectorKind::Class(class_name.to_string());
                }
            }
            SelectorKind::Unsupported
        }
    }
}

fn strip_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            // Find end of comment
            i += 2;
            while i + 1 < bytes.len() {
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    result
}

fn extract_rules(input: &str) -> Vec<(String, String)> {
    let mut rules = Vec::new();
    let mut rest = input;

    while let Some(open) = rest.find('{') {
        let selector = rest[..open].to_string();
        let after_open = &rest[open + 1..];
        if let Some(close) = after_open.find('}') {
            let body = after_open[..close].to_string();
            rules.push((selector, body));
            rest = &after_open[close + 1..];
        } else {
            break;
        }
    }

    rules
}

fn parse_declarations(body: &str) -> CssStyle {
    let mut style = CssStyle::default();

    for decl in body.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        if let Some((key, value)) = decl.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            apply_property(&mut style, key, value);
        }
    }

    style
}

fn apply_property(style: &mut CssStyle, key: &str, value: &str) {
    match key {
        "color" => {
            if let Some(hex) = parse_hex_color(value) {
                style.color = Some(hex);
            } else {
                eprintln!("warning: Unsupported value `{}` for `color`", value);
            }
        }
        "background-color" => {
            if let Some(hex) = parse_hex_color(value) {
                style.background_color = Some(hex);
            } else {
                eprintln!(
                    "warning: Unsupported value `{}` for `background-color`",
                    value
                );
            }
        }
        "font-weight" => match value {
            "bold" => style.bold = Some(true),
            "normal" => style.bold = Some(false),
            _ => eprintln!("warning: Unsupported value `{}` for `font-weight`", value),
        },
        "font-style" => match value {
            "italic" => style.italic = Some(true),
            "normal" => style.italic = Some(false),
            _ => eprintln!("warning: Unsupported value `{}` for `font-style`", value),
        },
        "text-decoration" => match value {
            "underline" => style.underline = Some(true),
            "none" => style.underline = Some(false),
            _ => eprintln!(
                "warning: Unsupported value `{}` for `text-decoration`",
                value
            ),
        },
        "font-family" => {
            // フォントスタック対応: "Courier New", Courier, monospace → "Courier New" を抽出
            let first = value.split(',').next().unwrap_or("").trim();
            let family = first.trim_matches(|c| c == '"' || c == '\'');
            if !family.is_empty() {
                style.font_family = Some(family.to_string());
            }
        }
        "padding" => {
            if let Some(pt) = parse_pt_value(value) {
                style.padding_top_pt = Some(pt);
                style.padding_right_pt = Some(pt);
                style.padding_bottom_pt = Some(pt);
                style.padding_left_pt = Some(pt);
            } else {
                eprintln!(
                    "warning: Unsupported value `{}` for `padding` (use pt units)",
                    value
                );
            }
        }
        "padding-top" => {
            if let Some(pt) = parse_pt_value(value) {
                style.padding_top_pt = Some(pt);
            } else {
                eprintln!(
                    "warning: Unsupported value `{}` for `padding-top` (use pt units)",
                    value
                );
            }
        }
        "padding-right" => {
            if let Some(pt) = parse_pt_value(value) {
                style.padding_right_pt = Some(pt);
            } else {
                eprintln!(
                    "warning: Unsupported value `{}` for `padding-right` (use pt units)",
                    value
                );
            }
        }
        "padding-bottom" => {
            if let Some(pt) = parse_pt_value(value) {
                style.padding_bottom_pt = Some(pt);
            } else {
                eprintln!(
                    "warning: Unsupported value `{}` for `padding-bottom` (use pt units)",
                    value
                );
            }
        }
        "padding-left" => {
            if let Some(pt) = parse_pt_value(value) {
                style.padding_left_pt = Some(pt);
            } else {
                eprintln!(
                    "warning: Unsupported value `{}` for `padding-left` (use pt units)",
                    value
                );
            }
        }
        "font-size" => {
            if let Some(pt) = parse_pt_value(value) {
                style.font_size_pt = Some(pt);
            } else {
                eprintln!("warning: Unsupported value `{}` for `font-size`", value);
            }
        }
        "line-height" => {
            if let Some(pt) = parse_pt_value(value) {
                style.line_height = Some(pt);
            } else {
                eprintln!("warning: Unsupported value `{}` for `line-height` (use pt units, e.g. 18pt)", value);
            }
        }
        "border" => {
            if let Some(b) = parse_border_value(value) {
                style.border_top = Some(b.clone());
                style.border_bottom = Some(b.clone());
                style.border_left = Some(b.clone());
                style.border_right = Some(b);
            } else {
                eprintln!("warning: Unsupported value `{}` for `border`", value);
            }
        }
        "border-top" => {
            if let Some(b) = parse_border_value(value) {
                style.border_top = Some(b);
            } else {
                eprintln!("warning: Unsupported value `{}` for `border-top`", value);
            }
        }
        "border-bottom" => {
            if let Some(b) = parse_border_value(value) {
                style.border_bottom = Some(b);
            } else {
                eprintln!(
                    "warning: Unsupported value `{}` for `border-bottom`",
                    value
                );
            }
        }
        "border-left" => {
            if let Some(b) = parse_border_value(value) {
                style.border_left = Some(b);
            } else {
                eprintln!("warning: Unsupported value `{}` for `border-left`", value);
            }
        }
        "border-right" => {
            if let Some(b) = parse_border_value(value) {
                style.border_right = Some(b);
            } else {
                eprintln!(
                    "warning: Unsupported value `{}` for `border-right`",
                    value
                );
            }
        }
        _ => {
            eprintln!("warning: Unsupported property `{}`", key);
        }
    }
}

fn parse_hex_color(value: &str) -> Option<String> {
    let s = value.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(hex.to_uppercase());
        }
    }
    None
}

fn parse_pt_value(value: &str) -> Option<f32> {
    let s = value.trim();
    if let Some(num) = s.strip_suffix("pt") {
        num.trim().parse::<f32>().ok()
    } else {
        None
    }
}

/// Parse CSS border shorthand: "2px double #000000" → CssBorder
fn parse_border_value(value: &str) -> Option<CssBorder> {
    let parts: Vec<&str> = value.trim().split_whitespace().collect();
    if parts.len() != 3 {
        return None;
    }

    // Parse size: "Npx"
    let size_px = parts[0].strip_suffix("px")?.parse::<f32>().ok()?;

    // Parse style
    let style = parts[1];
    match style {
        "solid" | "double" | "dotted" | "dashed" | "none" => {}
        _ => return None,
    }

    // Parse color: "#rrggbb"
    let color = parse_hex_color(parts[2])?;

    Some(CssBorder {
        style: style.to_string(),
        size_px,
        color,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_comments_removes_block_comments() {
        assert_eq!(strip_comments("a /* comment */ b"), "a  b");
        assert_eq!(strip_comments("/* start */h1{}"), "h1{}");
        assert_eq!(strip_comments("h1{/* mid */}"), "h1{}");
    }

    #[test]
    fn extract_rules_multiple() {
        let input = "h1 { color: red; } h2 { font-size: 12pt; }";
        let rules = extract_rules(input);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].0.trim(), "h1");
        assert_eq!(rules[1].0.trim(), "h2");
    }

    #[test]
    fn classify_selector_heading() {
        assert!(matches!(classify_selector("h1"), SelectorKind::H1));
        assert!(matches!(classify_selector("h2"), SelectorKind::H2));
        assert!(matches!(classify_selector("h3"), SelectorKind::H3));
        assert!(matches!(classify_selector("h4"), SelectorKind::H4));
        assert!(matches!(classify_selector("h5"), SelectorKind::H5));
    }

    #[test]
    fn classify_selector_table_th() {
        assert!(matches!(classify_selector("table th"), SelectorKind::TableTh));
    }

    #[test]
    fn classify_selector_class() {
        match classify_selector(".warning") {
            SelectorKind::Class(name) => assert_eq!(name, "warning"),
            _ => panic!("expected Class"),
        }
    }

    #[test]
    fn classify_selector_unsupported() {
        assert!(matches!(classify_selector("span.warning"), SelectorKind::Unsupported));
        assert!(matches!(classify_selector("#id"), SelectorKind::Unsupported));
        assert!(matches!(classify_selector("div"), SelectorKind::Unsupported));
    }

    #[test]
    fn parse_hex_color_valid() {
        assert_eq!(parse_hex_color("#c00000"), Some("C00000".to_string()));
        assert_eq!(parse_hex_color("#EEEEEE"), Some("EEEEEE".to_string()));
    }

    #[test]
    fn parse_hex_color_rejects_rgb() {
        assert_eq!(parse_hex_color("rgb(200, 0, 0)"), None);
    }

    #[test]
    fn parse_hex_color_rejects_shorthand() {
        assert_eq!(parse_hex_color("#ccc"), None);
    }

    #[test]
    fn parse_pt_value_valid() {
        assert_eq!(parse_pt_value("14pt"), Some(14.0));
        assert_eq!(parse_pt_value("10.5pt"), Some(10.5));
    }

    #[test]
    fn parse_pt_value_rejects_em() {
        assert_eq!(parse_pt_value("1.2em"), None);
    }

    #[test]
    fn parse_line_height_pt() {
        let rules = parse_css("h1 { line-height: 18pt; }");
        let h1 = rules.h1.unwrap();
        assert_eq!(h1.line_height, Some(18.0));
    }

    #[test]
    fn parse_declarations_color_and_bold() {
        let style = parse_declarations("color: #ff0000; font-weight: bold;");
        assert_eq!(style.color, Some("FF0000".to_string()));
        assert_eq!(style.bold, Some(true));
    }

    #[test]
    fn integration_full_parse() {
        let css = r#"
            h1 { font-size: 28pt; line-height: 36pt; }
            table th { background-color: #eeeeee; font-weight: bold; }
            .warning { color: #c00000; font-weight: bold; }
        "#;
        let rules = parse_css(css);

        let h1 = rules.h1.unwrap();
        assert_eq!(h1.font_size_pt, Some(28.0));
        assert_eq!(h1.line_height, Some(36.0));

        let th = rules.table_th.unwrap();
        assert_eq!(th.background_color, Some("EEEEEE".to_string()));
        assert_eq!(th.bold, Some(true));

        let warning = rules.classes.get("warning").unwrap();
        assert_eq!(warning.color, Some("C00000".to_string()));
        assert_eq!(warning.bold, Some(true));
    }

    #[test]
    fn unsupported_property_is_skipped() {
        let style = parse_declarations("margin: 10px; color: #ff0000;");
        assert_eq!(style.color, Some("FF0000".to_string()));
        // margin is just ignored (warning printed to stderr)
    }

    #[test]
    fn font_family_strips_quotes() {
        let style = parse_declarations("font-family: \"Arial\";");
        assert_eq!(style.font_family, Some("Arial".to_string()));

        let style2 = parse_declarations("font-family: 'Helvetica';");
        assert_eq!(style2.font_family, Some("Helvetica".to_string()));
    }

    #[test]
    fn font_style_italic() {
        let style = parse_declarations("font-style: italic;");
        assert_eq!(style.italic, Some(true));
    }

    #[test]
    fn text_decoration_underline() {
        let style = parse_declarations("text-decoration: underline;");
        assert_eq!(style.underline, Some(true));
    }

    #[test]
    fn border_shorthand_sets_all_sides() {
        let style = parse_declarations("border: 2px double #000000;");
        let top = style.border_top.unwrap();
        assert_eq!(top.style, "double");
        assert_eq!(top.size_px, 2.0);
        assert_eq!(top.color, "000000");
        assert!(style.border_bottom.is_some());
        assert!(style.border_left.is_some());
        assert!(style.border_right.is_some());
    }

    #[test]
    fn border_individual_sides() {
        let style = parse_declarations("border-top: 1px solid #FF0000; border-bottom: 3px dashed #00FF00;");
        let top = style.border_top.unwrap();
        assert_eq!(top.style, "solid");
        assert_eq!(top.size_px, 1.0);
        assert_eq!(top.color, "FF0000");
        let bottom = style.border_bottom.unwrap();
        assert_eq!(bottom.style, "dashed");
        assert_eq!(bottom.size_px, 3.0);
        assert_eq!(bottom.color, "00FF00");
        assert!(style.border_left.is_none());
        assert!(style.border_right.is_none());
    }

    #[test]
    fn border_none_style() {
        let style = parse_declarations("border: 0px none #000000;");
        let top = style.border_top.unwrap();
        assert_eq!(top.style, "none");
    }

    #[test]
    fn border_dotted_style() {
        let style = parse_declarations("border: 1px dotted #333333;");
        let top = style.border_top.unwrap();
        assert_eq!(top.style, "dotted");
    }

    #[test]
    fn border_rejects_invalid_value() {
        let style = parse_declarations("border: thick;");
        assert!(style.border_top.is_none());
    }

    #[test]
    fn border_rejects_unsupported_style() {
        let style = parse_declarations("border: 2px groove #000000;");
        assert!(style.border_top.is_none());
    }

    #[test]
    fn classify_selector_code_block() {
        assert!(matches!(classify_selector("pre"), SelectorKind::CodeBlock));
        assert!(matches!(classify_selector("code"), SelectorKind::CodeBlock));
        assert!(matches!(
            classify_selector("pre code"),
            SelectorKind::CodeBlock
        ));
    }

    #[test]
    fn classify_selector_code_block_comma() {
        assert!(matches!(
            classify_selector("pre, code"),
            SelectorKind::CodeBlock
        ));
    }

    #[test]
    fn classify_selector_blockquote() {
        assert!(matches!(
            classify_selector("blockquote"),
            SelectorKind::Blockquote
        ));
    }

    #[test]
    fn parse_code_block_css() {
        let css = r#"pre { font-size: 8pt; background-color: #f0f0f0; }"#;
        let rules = parse_css(css);
        let cb = rules.code_block.unwrap();
        assert_eq!(cb.font_size_pt, Some(8.0));
        assert_eq!(cb.background_color, Some("F0F0F0".to_string()));
    }

    #[test]
    fn parse_blockquote_css() {
        let css = r#"blockquote { color: #666666; border-left: 3px solid #cccccc; }"#;
        let rules = parse_css(css);
        let bq = rules.blockquote.unwrap();
        assert_eq!(bq.color, Some("666666".to_string()));
        let bl = bq.border_left.unwrap();
        assert_eq!(bl.style, "solid");
        assert_eq!(bl.size_px, 3.0);
        assert_eq!(bl.color, "CCCCCC");
    }

    #[test]
    fn font_family_font_stack_extracts_first() {
        let style = parse_declarations("font-family: \"Courier New\", Courier, monospace;");
        assert_eq!(style.font_family, Some("Courier New".to_string()));
    }

    #[test]
    fn font_family_single_unquoted() {
        let style = parse_declarations("font-family: monospace;");
        assert_eq!(style.font_family, Some("monospace".to_string()));
    }

    #[test]
    fn padding_left_parsed() {
        let style = parse_declarations("padding-left: 12pt;");
        assert_eq!(style.padding_left_pt, Some(12.0));
    }

    #[test]
    fn padding_left_rejects_px() {
        let style = parse_declarations("padding-left: 10px;");
        assert!(style.padding_left_pt.is_none());
    }

    #[test]
    fn padding_shorthand_sets_all_sides() {
        let style = parse_declarations("padding: 8pt;");
        assert_eq!(style.padding_top_pt, Some(8.0));
        assert_eq!(style.padding_right_pt, Some(8.0));
        assert_eq!(style.padding_bottom_pt, Some(8.0));
        assert_eq!(style.padding_left_pt, Some(8.0));
    }

    #[test]
    fn padding_individual_sides() {
        let style = parse_declarations(
            "padding-top: 4pt; padding-right: 6pt; padding-bottom: 8pt; padding-left: 10pt;",
        );
        assert_eq!(style.padding_top_pt, Some(4.0));
        assert_eq!(style.padding_right_pt, Some(6.0));
        assert_eq!(style.padding_bottom_pt, Some(8.0));
        assert_eq!(style.padding_left_pt, Some(10.0));
    }
}
