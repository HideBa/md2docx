use std::sync::LazyLock;

use anyhow::Result;
use mitex_parser::syntax::{CmdItem, EnvItem, LRItem, SyntaxElement, SyntaxKind, SyntaxNode};
use mitex_parser::CommandSpec;
use mitex_spec::preludes::command::*;
use mitex_spec::{ArgPattern, CmdShape, CommandSpecItem, ContextFeature, EnvShape};
use rowan::ast::AstNode;

static MATH_SPEC: LazyLock<CommandSpec> = LazyLock::new(build_math_spec);

/// LaTeX 数式文字列を OMML XML に変換する。
/// `display` が true ならディスプレイ数式（`<m:oMathPara>`でラップ）。
pub fn latex_to_omml(latex: &str, display: bool) -> Result<String> {
    let root = mitex_parser::parse(latex, MATH_SPEC.clone());
    let mut buf = String::new();
    if display {
        buf.push_str("<m:oMathPara><m:oMath>");
    } else {
        buf.push_str("<m:oMath>");
    }
    convert_node(&root, &mut buf);
    if display {
        buf.push_str("</m:oMath></m:oMathPara>");
    } else {
        buf.push_str("</m:oMath>");
    }
    Ok(buf)
}

fn convert_node(node: &SyntaxNode, buf: &mut String) {
    for child in node.children_with_tokens() {
        match child.kind() {
            SyntaxKind::TokenWord => {
                let text = child.as_token().unwrap().text();
                for ch in text.chars() {
                    write_run(buf, ch, !ch.is_ascii_digit());
                }
            }
            SyntaxKind::TokenCommandSym => {
                let text = child.as_token().unwrap().text();
                let name = &text[1..]; // remove leading '\'
                if let Some(sym) = latex_symbol_to_unicode(name) {
                    write_run_str(buf, sym, false);
                } else {
                    // Unknown symbol: output as text
                    write_run_str(buf, text, false);
                }
            }
            SyntaxKind::ItemCmd => {
                let cmd_node = child.as_node().unwrap();
                convert_cmd(cmd_node, buf);
            }
            SyntaxKind::ItemAttachComponent => {
                let node = child.as_node().unwrap();
                convert_attach(node, buf);
            }
            SyntaxKind::ItemCurly => {
                // Curly braces group: just process contents
                let node = child.as_node().unwrap();
                convert_curly_contents(node, buf);
            }
            SyntaxKind::ItemLR => {
                let node = child.as_node().unwrap();
                convert_lr(node, buf);
            }
            SyntaxKind::ItemEnv => {
                let node = child.as_node().unwrap();
                convert_env(node, buf);
            }
            SyntaxKind::ScopeRoot
            | SyntaxKind::ItemText
            | SyntaxKind::ClauseArgument
            | SyntaxKind::ItemBracket
            | SyntaxKind::ItemParen => {
                let node = child.as_node().unwrap();
                convert_node(node, buf);
            }
            SyntaxKind::TokenLBrace
            | SyntaxKind::TokenRBrace
            | SyntaxKind::TokenDollar
            | SyntaxKind::TokenBeginMath
            | SyntaxKind::TokenEndMath
            | SyntaxKind::TokenWhiteSpace
            | SyntaxKind::TokenLineBreak
            | SyntaxKind::TokenComment => {
                // Skip structural/whitespace tokens
            }
            SyntaxKind::TokenLParen => write_run(buf, '(', false),
            SyntaxKind::TokenRParen => write_run(buf, ')', false),
            SyntaxKind::TokenLBracket => write_run(buf, '[', false),
            SyntaxKind::TokenRBracket => write_run(buf, ']', false),
            SyntaxKind::TokenComma => write_run(buf, ',', false),
            SyntaxKind::TokenSemicolon => write_run(buf, ';', false),
            SyntaxKind::TokenAsterisk => write_run(buf, '*', false),
            SyntaxKind::TokenSlash => write_run(buf, '/', false),
            SyntaxKind::TokenTilde => write_run(buf, '\u{00A0}', false), // non-breaking space
            SyntaxKind::TokenHash => write_run(buf, '#', false),
            SyntaxKind::TokenApostrophe => {
                // ' is a prime in math mode
                write_run(buf, '\u{2032}', false);
            }
            SyntaxKind::TokenUnderscore | SyntaxKind::TokenCaret => {
                // Handled by ItemAttachComponent
            }
            _ => {
                // Fallback: try to process as node
                if let Some(node) = child.as_node() {
                    convert_node(node, buf);
                }
            }
        }
    }
}

fn convert_cmd(node: &SyntaxNode, buf: &mut String) {
    let Some(cmd) = CmdItem::cast(node.clone()) else {
        return;
    };
    let Some(name_tok) = cmd.name_tok() else {
        return;
    };
    let name = &name_tok.text()[1..]; // remove leading '\'
    let args: Vec<SyntaxNode> = cmd.arguments().collect();

    match name {
        // Fractions
        "frac" => {
            if args.len() >= 2 {
                buf.push_str("<m:f><m:num>");
                convert_node(&args[0], buf);
                buf.push_str("</m:num><m:den>");
                convert_node(&args[1], buf);
                buf.push_str("</m:den></m:f>");
            }
        }
        // Square root
        "sqrt" => {
            if args.len() >= 2 {
                // \sqrt[n]{x}
                buf.push_str("<m:rad><m:radPr><m:degHide m:val=\"0\"/></m:radPr><m:deg>");
                convert_node(&args[0], buf);
                buf.push_str("</m:deg><m:e>");
                convert_node(&args[1], buf);
                buf.push_str("</m:e></m:rad>");
            } else if args.len() == 1 {
                // \sqrt{x}
                buf.push_str(
                    "<m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e>",
                );
                convert_node(&args[0], buf);
                buf.push_str("</m:e></m:rad>");
            }
        }
        // Binomial
        "binom" => {
            if args.len() >= 2 {
                buf.push_str("<m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:f><m:fPr><m:type m:val=\"noBar\"/></m:fPr><m:num>");
                convert_node(&args[0], buf);
                buf.push_str("</m:num><m:den>");
                convert_node(&args[1], buf);
                buf.push_str("</m:den></m:f></m:e></m:d>");
            }
        }
        // Overset / Underset
        "overset" => {
            if args.len() >= 2 {
                buf.push_str("<m:limUpp><m:e>");
                convert_node(&args[1], buf);
                buf.push_str("</m:e><m:lim>");
                convert_node(&args[0], buf);
                buf.push_str("</m:lim></m:limUpp>");
            }
        }
        "underset" => {
            if args.len() >= 2 {
                buf.push_str("<m:limLow><m:e>");
                convert_node(&args[1], buf);
                buf.push_str("</m:e><m:lim>");
                convert_node(&args[0], buf);
                buf.push_str("</m:lim></m:limLow>");
            }
        }
        // Accents and decorations
        "hat" => write_accent(buf, "\u{0302}", &args),
        "bar" | "overline" => write_accent(buf, "\u{0305}", &args),
        "dot" => write_accent(buf, "\u{0307}", &args),
        "ddot" => write_accent(buf, "\u{0308}", &args),
        "tilde" | "widetilde" => write_accent(buf, "\u{0303}", &args),
        "vec" | "overrightarrow" => write_accent(buf, "\u{20D7}", &args),
        "widehat" => write_accent(buf, "\u{0302}", &args),
        "overleftarrow" => write_accent(buf, "\u{20D6}", &args),
        "underline" => {
            if let Some(arg) = args.first() {
                buf.push_str("<m:groupChr><m:groupChrPr><m:chr m:val=\"\u{0332}\"/><m:pos m:val=\"bot\"/></m:groupChrPr><m:e>");
                convert_node(arg, buf);
                buf.push_str("</m:e></m:groupChr>");
            }
        }
        // Font commands
        "mathrm" | "textrm" | "text" => {
            if let Some(arg) = args.first() {
                write_text_run(buf, arg, "nor");
            }
        }
        "mathbf" | "textbf" | "boldsymbol" => {
            if let Some(arg) = args.first() {
                write_text_run(buf, arg, "b");
            }
        }
        "mathit" | "textit" => {
            if let Some(arg) = args.first() {
                write_text_run(buf, arg, "i");
            }
        }
        "mathbb" => {
            if let Some(arg) = args.first() {
                write_text_run(buf, arg, "double-struck");
            }
        }
        "mathcal" => {
            if let Some(arg) = args.first() {
                write_text_run(buf, arg, "script");
            }
        }
        "mathfrak" => {
            if let Some(arg) = args.first() {
                write_text_run(buf, arg, "fraktur");
            }
        }
        "mathsf" => {
            if let Some(arg) = args.first() {
                write_text_run(buf, arg, "sans-serif");
            }
        }
        "mathtt" => {
            if let Some(arg) = args.first() {
                write_text_run(buf, arg, "monospace");
            }
        }
        "operatorname" => {
            if let Some(arg) = args.first() {
                write_text_run(buf, arg, "nor");
            }
        }
        "not" => {
            // \not followed by a symbol — combine with slash
            if let Some(arg) = args.first() {
                let text = arg.text().to_string();
                let text = text.trim();
                if let Some(sym) = latex_symbol_to_unicode(text.trim_start_matches('\\')) {
                    write_run_str(buf, &format!("{sym}\u{0338}"), false);
                } else {
                    convert_node(arg, buf);
                }
            }
        }
        // Large operators — just output the symbol, subscripts/superscripts handled by ItemAttachComponent
        _ if is_nary_operator(name) => {
            if let Some(sym) = latex_symbol_to_unicode(name) {
                write_run_str(buf, sym, false);
            } else {
                write_run_str(buf, name, false);
            }
        }
        // Symbols (no-arg commands)
        _ => {
            if let Some(sym) = latex_symbol_to_unicode(name) {
                write_run_str(buf, sym, false);
            } else {
                // Fallback: output command name
                write_run_str(buf, &format!("\\{name}"), false);
            }
        }
    }
}

fn convert_attach(node: &SyntaxNode, buf: &mut String) {
    let mut base_elements: Vec<SyntaxElement> = Vec::new();
    let mut sub_element: Option<SyntaxElement> = None;
    let mut sup_element: Option<SyntaxElement> = None;
    let mut current_is_sub = false;
    let mut current_is_sup = false;

    for part in node.children_with_tokens() {
        match part.kind() {
            SyntaxKind::TokenUnderscore => {
                current_is_sub = true;
                current_is_sup = false;
            }
            SyntaxKind::TokenCaret => {
                current_is_sup = true;
                current_is_sub = false;
            }
            SyntaxKind::TokenWhiteSpace | SyntaxKind::TokenLineBreak => {}
            _ => {
                if current_is_sub {
                    sub_element = Some(part);
                    current_is_sub = false;
                } else if current_is_sup {
                    sup_element = Some(part);
                    current_is_sup = false;
                } else {
                    base_elements.push(part);
                }
            }
        }
    }

    // Check if base contains a nary operator (possibly nested in ClauseArgument/ItemAttachComponent)
    let nary_info = find_nary_in_base(&base_elements);

    // Handle nested attach: e.g. \sum_{i=0}^{n} parses as
    //   outer_attach(base=inner_attach(\sum, sub=i=0), sup=n)
    // We merge them into a single nary expression.
    if let Some(nary_char) = nary_info {
        // Collect sub/sup from nested attach components and merge with outer
        let mut sub_buf = String::new();
        let mut sup_buf = String::new();
        collect_nested_nary_limits(&base_elements, &mut sub_buf, &mut sup_buf);
        // Outer limits override if present
        if let Some(ref sub) = sub_element {
            if sub_buf.is_empty() {
                convert_element(sub, &mut sub_buf);
            }
        }
        if let Some(ref sup) = sup_element {
            if sup_buf.is_empty() {
                convert_element(sup, &mut sup_buf);
            }
        }

        buf.push_str("<m:nary><m:naryPr>");
        buf.push_str(&format!("<m:chr m:val=\"{nary_char}\"/>"));
        buf.push_str("<m:limLoc m:val=\"undOvr\"/>");
        if sub_buf.is_empty() {
            buf.push_str("<m:subHide m:val=\"1\"/>");
        }
        if sup_buf.is_empty() {
            buf.push_str("<m:supHide m:val=\"1\"/>");
        }
        buf.push_str("</m:naryPr>");

        buf.push_str("<m:sub>");
        buf.push_str(&sub_buf);
        buf.push_str("</m:sub>");

        buf.push_str("<m:sup>");
        buf.push_str(&sup_buf);
        buf.push_str("</m:sup>");

        buf.push_str("<m:e/></m:nary>");
    } else if sub_element.is_some() && sup_element.is_some() {
        // Both sub and sup: <m:sSubSup>
        buf.push_str("<m:sSubSup><m:e>");
        for e in &base_elements {
            convert_element(e, buf);
        }
        buf.push_str("</m:e><m:sub>");
        convert_element(sub_element.as_ref().unwrap(), buf);
        buf.push_str("</m:sub><m:sup>");
        convert_element(sup_element.as_ref().unwrap(), buf);
        buf.push_str("</m:sup></m:sSubSup>");
    } else if let Some(ref sub) = sub_element {
        // Subscript only
        buf.push_str("<m:sSub><m:e>");
        for e in &base_elements {
            convert_element(e, buf);
        }
        buf.push_str("</m:e><m:sub>");
        convert_element(sub, buf);
        buf.push_str("</m:sub></m:sSub>");
    } else if let Some(ref sup) = sup_element {
        // Superscript only
        buf.push_str("<m:sSup><m:e>");
        for e in &base_elements {
            convert_element(e, buf);
        }
        buf.push_str("</m:e><m:sup>");
        convert_element(sup, buf);
        buf.push_str("</m:sup></m:sSup>");
    } else {
        // No sub/sup — just output base
        for e in &base_elements {
            convert_element(e, buf);
        }
    }
}

fn convert_element(elem: &SyntaxElement, buf: &mut String) {
    match elem.kind() {
        // Dispatch structured nodes to their dedicated handlers
        SyntaxKind::ItemCmd => convert_cmd(elem.as_node().unwrap(), buf),
        SyntaxKind::ItemAttachComponent => convert_attach(elem.as_node().unwrap(), buf),
        SyntaxKind::ItemLR => convert_lr(elem.as_node().unwrap(), buf),
        SyntaxKind::ItemEnv => convert_env(elem.as_node().unwrap(), buf),
        SyntaxKind::ItemCurly => convert_curly_contents(elem.as_node().unwrap(), buf),
        // Tokens
        SyntaxKind::TokenWord => {
            let token = elem.as_token().unwrap();
            for ch in token.text().chars() {
                write_run(buf, ch, !ch.is_ascii_digit());
            }
        }
        SyntaxKind::TokenCommandSym => {
            let token = elem.as_token().unwrap();
            let name = &token.text()[1..];
            if let Some(sym) = latex_symbol_to_unicode(name) {
                write_run_str(buf, sym, false);
            } else {
                write_run_str(buf, token.text(), false);
            }
        }
        // Other tokens
        SyntaxKind::TokenLParen => write_run(buf, '(', false),
        SyntaxKind::TokenRParen => write_run(buf, ')', false),
        SyntaxKind::TokenLBracket => write_run(buf, '[', false),
        SyntaxKind::TokenRBracket => write_run(buf, ']', false),
        SyntaxKind::TokenComma => write_run(buf, ',', false),
        SyntaxKind::TokenSemicolon => write_run(buf, ';', false),
        SyntaxKind::TokenAsterisk => write_run(buf, '*', false),
        SyntaxKind::TokenSlash => write_run(buf, '/', false),
        SyntaxKind::TokenTilde => write_run(buf, '\u{00A0}', false),
        SyntaxKind::TokenHash => write_run(buf, '#', false),
        SyntaxKind::TokenApostrophe => write_run(buf, '\u{2032}', false),
        // Structural/whitespace tokens: skip
        SyntaxKind::TokenLBrace
        | SyntaxKind::TokenRBrace
        | SyntaxKind::TokenDollar
        | SyntaxKind::TokenBeginMath
        | SyntaxKind::TokenEndMath
        | SyntaxKind::TokenWhiteSpace
        | SyntaxKind::TokenLineBreak
        | SyntaxKind::TokenComment
        | SyntaxKind::TokenUnderscore
        | SyntaxKind::TokenCaret => {}
        // Other nodes: recurse into children
        _ => {
            if let Some(node) = elem.as_node() {
                convert_node(node, buf);
            }
        }
    }
}

fn convert_lr(node: &SyntaxNode, buf: &mut String) {
    let Some(lr) = LRItem::cast(node.clone()) else {
        convert_node(node, buf);
        return;
    };

    let left_sym = lr
        .left_sym()
        .map(|t| normalize_delimiter(t.text()))
        .unwrap_or_default();
    let right_sym = lr
        .right_sym()
        .map(|t| normalize_delimiter(t.text()))
        .unwrap_or_default();

    buf.push_str("<m:d><m:dPr>");
    buf.push_str(&format!("<m:begChr m:val=\"{left_sym}\"/>"));
    buf.push_str(&format!("<m:endChr m:val=\"{right_sym}\"/>"));
    buf.push_str("</m:dPr><m:e>");

    // Process content between \left and \right (skip ClauseLR nodes)
    for child in node.children_with_tokens() {
        match child.kind() {
            SyntaxKind::ClauseLR
            | SyntaxKind::TokenLBrace
            | SyntaxKind::TokenRBrace => {}
            _ => convert_element(&child, buf),
        }
    }

    buf.push_str("</m:e></m:d>");
}

fn convert_env(node: &SyntaxNode, buf: &mut String) {
    let Some(env) = EnvItem::cast(node.clone()) else {
        convert_node(node, buf);
        return;
    };
    let env_name = env
        .name_tok()
        .map(|t| t.text().to_string())
        .unwrap_or_default();

    match env_name.as_str() {
        "matrix" | "pmatrix" | "bmatrix" | "Bmatrix" | "vmatrix" | "Vmatrix" | "smallmatrix" => {
            let (open, close) = match env_name.as_str() {
                "pmatrix" => ("(", ")"),
                "bmatrix" => ("[", "]"),
                "Bmatrix" => ("{", "}"),
                "vmatrix" => ("|", "|"),
                "Vmatrix" => ("\u{2016}", "\u{2016}"),
                _ => ("", ""), // matrix, smallmatrix: no delimiters
            };

            let has_delimiters = !open.is_empty();
            if has_delimiters {
                buf.push_str("<m:d><m:dPr>");
                buf.push_str(&format!("<m:begChr m:val=\"{open}\"/>"));
                buf.push_str(&format!("<m:endChr m:val=\"{close}\"/>"));
                buf.push_str("</m:dPr><m:e>");
            }

            convert_matrix_body(node, buf);

            if has_delimiters {
                buf.push_str("</m:e></m:d>");
            }
        }
        "cases" => {
            buf.push_str("<m:d><m:dPr>");
            buf.push_str("<m:begChr m:val=\"{\"/>");
            buf.push_str("<m:endChr m:val=\"\"/>");
            buf.push_str("</m:dPr><m:e>");
            convert_matrix_body(node, buf);
            buf.push_str("</m:e></m:d>");
        }
        _ => {
            // Other environments: just process contents
            convert_node(node, buf);
        }
    }
}

fn convert_matrix_body(node: &SyntaxNode, buf: &mut String) {
    // Parse matrix content: rows separated by ItemNewLine, cells by TokenAmpersand
    let mut rows: Vec<Vec<Vec<SyntaxElement>>> = Vec::new();
    let mut current_row: Vec<Vec<SyntaxElement>> =
        Vec::new();
    let mut current_cell: Vec<SyntaxElement> = Vec::new();

    for child in node.children_with_tokens() {
        match child.kind() {
            SyntaxKind::ItemBegin | SyntaxKind::ItemEnd => {}
            SyntaxKind::TokenAmpersand => {
                current_row.push(std::mem::take(&mut current_cell));
            }
            SyntaxKind::ItemNewLine => {
                current_row.push(std::mem::take(&mut current_cell));
                rows.push(std::mem::take(&mut current_row));
            }
            SyntaxKind::TokenWhiteSpace | SyntaxKind::TokenLineBreak => {}
            _ => {
                current_cell.push(child);
            }
        }
    }
    // Push remaining
    if !current_cell.is_empty() {
        current_row.push(current_cell);
    }
    if !current_row.is_empty() {
        rows.push(current_row);
    }

    buf.push_str("<m:m>");
    for row in &rows {
        buf.push_str("<m:mr>");
        for cell in row {
            buf.push_str("<m:e>");
            for elem in cell {
                convert_element(elem, buf);
            }
            buf.push_str("</m:e>");
        }
        buf.push_str("</m:mr>");
    }
    buf.push_str("</m:m>");
}

/// Process contents of a curly group, skipping braces
fn convert_curly_contents(node: &SyntaxNode, buf: &mut String) {
    for child in node.children_with_tokens() {
        match child.kind() {
            SyntaxKind::TokenLBrace | SyntaxKind::TokenRBrace => {}
            _ => convert_element(&child, buf),
        }
    }
}

// === Helper functions ===

fn write_run(buf: &mut String, ch: char, italic: bool) {
    buf.push_str("<m:r>");
    if italic {
        buf.push_str("<m:rPr><m:sty m:val=\"i\"/></m:rPr>");
    }
    buf.push_str("<m:t>");
    write_xml_escaped(buf, &ch.to_string());
    buf.push_str("</m:t></m:r>");
}

fn write_run_str(buf: &mut String, s: &str, italic: bool) {
    buf.push_str("<m:r>");
    if italic {
        buf.push_str("<m:rPr><m:sty m:val=\"i\"/></m:rPr>");
    }
    buf.push_str("<m:t>");
    write_xml_escaped(buf, s);
    buf.push_str("</m:t></m:r>");
}

fn write_xml_escaped(buf: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '&' => buf.push_str("&amp;"),
            '<' => buf.push_str("&lt;"),
            '>' => buf.push_str("&gt;"),
            '"' => buf.push_str("&quot;"),
            _ => buf.push(ch),
        }
    }
}

fn write_accent(buf: &mut String, accent_char: &str, args: &[SyntaxNode]) {
    if let Some(arg) = args.first() {
        buf.push_str("<m:acc><m:accPr>");
        buf.push_str(&format!("<m:chr m:val=\"{accent_char}\"/>"));
        buf.push_str("</m:accPr><m:e>");
        convert_node(arg, buf);
        buf.push_str("</m:e></m:acc>");
    }
}

fn write_text_run(buf: &mut String, node: &SyntaxNode, style: &str) {
    let text = extract_text(node);
    buf.push_str("<m:r><m:rPr>");
    buf.push_str(&format!("<m:sty m:val=\"{style}\"/>"));
    buf.push_str("</m:rPr><m:t>");
    write_xml_escaped(buf, &text);
    buf.push_str("</m:t></m:r>");
}

fn extract_text(node: &SyntaxNode) -> String {
    let mut s = String::new();
    for child in node.children_with_tokens() {
        match child.kind() {
            SyntaxKind::TokenWord => s.push_str(child.as_token().unwrap().text()),
            SyntaxKind::TokenWhiteSpace => s.push(' '),
            SyntaxKind::TokenCommandSym => {
                let name = &child.as_token().unwrap().text()[1..];
                if let Some(sym) = latex_symbol_to_unicode(name) {
                    s.push_str(sym);
                } else {
                    s.push_str(child.as_token().unwrap().text());
                }
            }
            SyntaxKind::TokenLBrace | SyntaxKind::TokenRBrace => {}
            _ => {
                if let Some(n) = child.as_node() {
                    s.push_str(&extract_text(n));
                }
            }
        }
    }
    s
}

/// Recursively search base elements for a nary operator, returning its Unicode symbol.
fn find_nary_in_base(base_elements: &[SyntaxElement]) -> Option<&'static str> {
    for e in base_elements {
        match e.kind() {
            SyntaxKind::ItemCmd => {
                if let Some(cmd) = CmdItem::cast(e.as_node().unwrap().clone()) {
                    if let Some(name_tok) = cmd.name_tok() {
                        let name = &name_tok.text()[1..];
                        if is_nary_operator(name) {
                            return latex_symbol_to_unicode(name);
                        }
                    }
                }
            }
            SyntaxKind::TokenCommandSym => {
                let name = &e.as_token().unwrap().text()[1..];
                if is_nary_operator(name) {
                    return latex_symbol_to_unicode(name);
                }
            }
            // Recurse into ClauseArgument or nested ItemAttachComponent
            SyntaxKind::ClauseArgument | SyntaxKind::ItemAttachComponent => {
                let children: Vec<SyntaxElement> =
                    e.as_node().unwrap().children_with_tokens().collect();
                if let Some(sym) = find_nary_in_base(&children) {
                    return Some(sym);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract sub/sup limits from nested attach components containing nary operators.
fn collect_nested_nary_limits(
    base_elements: &[SyntaxElement],
    sub_buf: &mut String,
    sup_buf: &mut String,
) {
    for e in base_elements {
        if e.kind() == SyntaxKind::ClauseArgument {
            let node = e.as_node().unwrap();
            for child in node.children() {
                if child.kind() == SyntaxKind::ItemAttachComponent {
                    // Parse inner attach's sub/sup
                    let mut is_sub = false;
                    let mut is_sup = false;
                    for part in child.children_with_tokens() {
                        match part.kind() {
                            SyntaxKind::TokenUnderscore => {
                                is_sub = true;
                                is_sup = false;
                            }
                            SyntaxKind::TokenCaret => {
                                is_sup = true;
                                is_sub = false;
                            }
                            SyntaxKind::TokenWhiteSpace | SyntaxKind::TokenLineBreak => {}
                            SyntaxKind::ClauseArgument => {
                                // This is the base of the inner attach — recurse if it also has nary
                                if !is_sub && !is_sup {
                                    // base: recurse further
                                    let children: Vec<SyntaxElement> =
                                        part.as_node().unwrap().children_with_tokens().collect();
                                    collect_nested_nary_limits(&children, sub_buf, sup_buf);
                                } else if is_sub {
                                    convert_element(&part, sub_buf);
                                    is_sub = false;
                                } else if is_sup {
                                    convert_element(&part, sup_buf);
                                    is_sup = false;
                                }
                            }
                            _ => {
                                if is_sub {
                                    convert_element(&part, sub_buf);
                                    is_sub = false;
                                } else if is_sup {
                                    convert_element(&part, sup_buf);
                                    is_sup = false;
                                }
                                // Skip base nary operator (already handled)
                            }
                        }
                    }
                }
            }
        }
    }
}

fn normalize_delimiter(s: &str) -> String {
    match s {
        "(" | "\\(" => "(".to_string(),
        ")" | "\\)" => ")".to_string(),
        "[" | "\\[" => "[".to_string(),
        "]" | "\\]" => "]".to_string(),
        "\\{" | "\\lbrace" => "{".to_string(),
        "\\}" | "\\rbrace" => "}".to_string(),
        "|" | "\\vert" => "|".to_string(),
        "\\|" | "\\Vert" => "\u{2016}".to_string(),
        "\\langle" => "\u{27E8}".to_string(),
        "\\rangle" => "\u{27E9}".to_string(),
        "\\lceil" => "\u{2308}".to_string(),
        "\\rceil" => "\u{2309}".to_string(),
        "\\lfloor" => "\u{230A}".to_string(),
        "\\rfloor" => "\u{230B}".to_string(),
        "." => "".to_string(), // invisible delimiter
        _ => s.to_string(),
    }
}

fn is_nary_operator(name: &str) -> bool {
    matches!(
        name,
        "sum"
            | "prod"
            | "coprod"
            | "int"
            | "iint"
            | "iiint"
            | "oint"
            | "bigcup"
            | "bigcap"
            | "bigoplus"
            | "bigotimes"
    )
}

fn latex_symbol_to_unicode(name: &str) -> Option<&'static str> {
    Some(match name {
        // Greek lowercase
        "alpha" => "\u{03B1}",
        "beta" => "\u{03B2}",
        "gamma" => "\u{03B3}",
        "delta" => "\u{03B4}",
        "epsilon" | "varepsilon" => "\u{03B5}",
        "zeta" => "\u{03B6}",
        "eta" => "\u{03B7}",
        "theta" => "\u{03B8}",
        "vartheta" => "\u{03D1}",
        "iota" => "\u{03B9}",
        "kappa" => "\u{03BA}",
        "lambda" => "\u{03BB}",
        "mu" => "\u{03BC}",
        "nu" => "\u{03BD}",
        "xi" => "\u{03BE}",
        "pi" => "\u{03C0}",
        "rho" => "\u{03C1}",
        "sigma" => "\u{03C3}",
        "tau" => "\u{03C4}",
        "upsilon" => "\u{03C5}",
        "phi" | "varphi" => "\u{03C6}",
        "chi" => "\u{03C7}",
        "psi" => "\u{03C8}",
        "omega" => "\u{03C9}",
        // Greek uppercase
        "Gamma" => "\u{0393}",
        "Delta" => "\u{0394}",
        "Theta" => "\u{0398}",
        "Lambda" => "\u{039B}",
        "Xi" => "\u{039E}",
        "Pi" => "\u{03A0}",
        "Sigma" => "\u{03A3}",
        "Upsilon" => "\u{03A5}",
        "Phi" => "\u{03A6}",
        "Psi" => "\u{03A8}",
        "Omega" => "\u{03A9}",
        // Operators
        "sum" => "\u{2211}",
        "prod" => "\u{220F}",
        "coprod" => "\u{2210}",
        "int" => "\u{222B}",
        "iint" => "\u{222C}",
        "iiint" => "\u{222D}",
        "oint" => "\u{222E}",
        "bigcup" => "\u{22C3}",
        "bigcap" => "\u{22C2}",
        "bigoplus" => "\u{2A01}",
        "bigotimes" => "\u{2A02}",
        // Relations
        "leq" | "le" => "\u{2264}",
        "geq" | "ge" => "\u{2265}",
        "neq" | "ne" => "\u{2260}",
        "approx" => "\u{2248}",
        "equiv" => "\u{2261}",
        "sim" => "\u{223C}",
        "simeq" => "\u{2243}",
        "cong" => "\u{2245}",
        "propto" => "\u{221D}",
        "ll" => "\u{226A}",
        "gg" => "\u{226B}",
        "prec" => "\u{227A}",
        "succ" => "\u{227B}",
        "preceq" => "\u{2AAF}",
        "succeq" => "\u{2AB0}",
        "perp" => "\u{22A5}",
        "parallel" => "\u{2225}",
        "mid" => "\u{2223}",
        // Set theory
        "subset" => "\u{2282}",
        "supset" => "\u{2283}",
        "subseteq" => "\u{2286}",
        "supseteq" => "\u{2287}",
        "in" => "\u{2208}",
        "notin" => "\u{2209}",
        "ni" => "\u{220B}",
        "cap" => "\u{2229}",
        "cup" => "\u{222A}",
        "setminus" => "\u{2216}",
        "emptyset" | "varnothing" => "\u{2205}",
        // Logic
        "forall" => "\u{2200}",
        "exists" => "\u{2203}",
        "nexists" => "\u{2204}",
        "neg" | "lnot" => "\u{00AC}",
        "vee" | "lor" => "\u{2228}",
        "wedge" | "land" => "\u{2227}",
        "implies" | "Rightarrow" => "\u{21D2}",
        "iff" | "Leftrightarrow" => "\u{21D4}",
        "Leftarrow" => "\u{21D0}",
        // Arrows
        "rightarrow" | "to" => "\u{2192}",
        "leftarrow" | "gets" => "\u{2190}",
        "leftrightarrow" => "\u{2194}",
        "uparrow" => "\u{2191}",
        "downarrow" => "\u{2193}",
        "mapsto" => "\u{21A6}",
        "hookrightarrow" => "\u{21AA}",
        "hookleftarrow" => "\u{21A9}",
        "longrightarrow" => "\u{27F6}",
        "longleftarrow" => "\u{27F5}",
        "Longrightarrow" => "\u{27F9}",
        "Longleftarrow" => "\u{27F8}",
        // Binary operations
        "times" => "\u{00D7}",
        "div" => "\u{00F7}",
        "pm" => "\u{00B1}",
        "mp" => "\u{2213}",
        "cdot" => "\u{22C5}",
        "star" => "\u{22C6}",
        "circ" => "\u{2218}",
        "bullet" => "\u{2022}",
        "oplus" => "\u{2295}",
        "otimes" => "\u{2297}",
        "odot" => "\u{2299}",
        "dagger" => "\u{2020}",
        "ddagger" => "\u{2021}",
        // Dots
        "cdots" => "\u{22EF}",
        "ldots" | "dots" => "\u{2026}",
        "vdots" => "\u{22EE}",
        "ddots" => "\u{22F1}",
        // Misc
        "infty" => "\u{221E}",
        "partial" => "\u{2202}",
        "nabla" => "\u{2207}",
        "hbar" => "\u{210F}",
        "ell" => "\u{2113}",
        "Re" => "\u{211C}",
        "Im" => "\u{2111}",
        "wp" => "\u{2118}",
        "aleph" => "\u{2135}",
        "angle" => "\u{2220}",
        "triangle" => "\u{25B3}",
        "diamond" => "\u{22C4}",
        "Box" => "\u{25A1}",
        // Brackets
        "langle" => "\u{27E8}",
        "rangle" => "\u{27E9}",
        "lceil" => "\u{2308}",
        "rceil" => "\u{2309}",
        "lfloor" => "\u{230A}",
        "rfloor" => "\u{230B}",
        "lbrace" => "{",
        "rbrace" => "}",
        "vert" => "|",
        "Vert" => "\u{2016}",
        // Spacing (map to thin/medium/thick space)
        "," => "\u{2009}",
        ";" => "\u{2005}",
        ":" | ">" => "\u{2005}",
        "!" => "",    // negative thin space
        "quad" => "\u{2003}",
        "qquad" => "\u{2003}\u{2003}",
        // Escaped literal characters
        "%" => "%",
        "#" => "#",
        "&" => "&",
        "$" => "$",
        "_" => "_",
        "{" => "{",
        "}" => "}",
        "\\" => "\n", // line break (in math rarely used standalone)
        // Trig/function names handled by text output, not here
        _ => return None,
    })
}

fn build_math_spec() -> CommandSpec {
    let mut b = SpecBuilder::default();

    // Symbols (no arguments)
    for name in [
        "alpha", "beta", "gamma", "delta", "epsilon", "varepsilon", "zeta", "eta", "theta",
        "vartheta", "iota", "kappa", "lambda", "mu", "nu", "xi", "pi", "rho", "sigma", "tau",
        "upsilon", "phi", "varphi", "chi", "psi", "omega", "Gamma", "Delta", "Theta", "Lambda",
        "Xi", "Pi", "Sigma", "Upsilon", "Phi", "Psi", "Omega", "infty", "partial", "nabla",
        "forall", "exists", "nexists", "emptyset", "varnothing", "cdot", "cdots", "ldots",
        "dots", "vdots", "ddots", "times", "div", "pm", "mp", "leq", "le", "geq", "ge", "neq",
        "ne", "approx", "equiv", "sim", "simeq", "cong", "propto", "ll", "gg", "prec", "succ",
        "preceq", "succeq", "perp", "parallel", "mid", "subset", "supset", "subseteq",
        "supseteq", "in", "notin", "ni", "cap", "cup", "setminus", "vee", "lor", "wedge",
        "land", "neg", "lnot", "implies", "iff", "rightarrow", "to", "leftarrow", "gets",
        "leftrightarrow", "Rightarrow", "Leftarrow", "Leftrightarrow", "uparrow", "downarrow",
        "mapsto", "hookrightarrow", "hookleftarrow", "longrightarrow", "longleftarrow",
        "Longrightarrow", "Longleftarrow", "star", "circ", "bullet", "oplus", "otimes", "odot",
        "dagger", "ddagger", "hbar", "ell", "Re", "Im", "wp", "aleph", "angle", "triangle",
        "diamond", "Box", "langle", "rangle", "lceil", "rceil", "lfloor", "rfloor", "lbrace",
        "rbrace", "vert", "Vert",
    ] {
        b.add_command(name, TEX_SYMBOL.clone());
    }

    // Large operators (symbols, subscripts handled by ItemAttachComponent)
    for name in [
        "sum", "prod", "coprod", "int", "iint", "iiint", "oint", "bigcup", "bigcap", "bigoplus",
        "bigotimes",
    ] {
        b.add_command(name, TEX_SYMBOL.clone());
    }

    // Function names (symbols)
    for name in [
        "lim", "limsup", "liminf", "sup", "inf", "max", "min", "sin", "cos", "tan", "cot",
        "sec", "csc", "log", "ln", "exp", "det", "dim", "ker", "hom", "arg", "deg", "gcd",
    ] {
        b.add_command(name, TEX_SYMBOL.clone());
    }

    // 1-argument commands
    for name in [
        "hat",
        "bar",
        "dot",
        "ddot",
        "tilde",
        "vec",
        "overline",
        "underline",
        "widehat",
        "widetilde",
        "overrightarrow",
        "overleftarrow",
        "mathbb",
        "mathcal",
        "mathfrak",
        "mathrm",
        "mathit",
        "mathbf",
        "mathsf",
        "mathtt",
        "text",
        "textrm",
        "textbf",
        "textit",
        "operatorname",
        "boldsymbol",
        "not",
    ] {
        b.add_command(name, TEX_CMD1.clone());
    }

    // 2-argument commands
    for name in ["frac", "binom", "overset", "underset"] {
        b.add_command(name, TEX_CMD2.clone());
    }

    // sqrt: glob pattern (optional bracket + term)
    b.add_command(
        "sqrt",
        CommandSpecItem::Cmd(CmdShape {
            args: mitex_spec::ArgShape::Right {
                pattern: ArgPattern::Glob {
                    pattern: "{,b}t".into(),
                },
            },
            alias: None,
        }),
    );

    // limits operator
    b.add_command(
        "limits",
        CommandSpecItem::Cmd(CmdShape {
            args: mitex_spec::ArgShape::Left1,
            alias: None,
        }),
    );

    // Environments
    for name in [
        "matrix",
        "pmatrix",
        "bmatrix",
        "Bmatrix",
        "vmatrix",
        "Vmatrix",
        "smallmatrix",
    ] {
        b.add_command(
            name,
            CommandSpecItem::Env(EnvShape {
                args: ArgPattern::None,
                ctx_feature: ContextFeature::IsMatrix,
                alias: None,
            }),
        );
    }

    b.add_command(
        "cases",
        CommandSpecItem::Env(EnvShape {
            args: ArgPattern::None,
            ctx_feature: ContextFeature::IsCases,
            alias: None,
        }),
    );

    for name in [
        "equation",
        "equation*",
        "align",
        "align*",
        "gather",
        "gather*",
        "aligned",
    ] {
        b.add_command(
            name,
            CommandSpecItem::Env(EnvShape {
                args: ArgPattern::None,
                ctx_feature: ContextFeature::IsMath,
                alias: None,
            }),
        );
    }

    // Greedy operators
    for name in ["displaystyle", "textstyle"] {
        b.add_command(
            name,
            CommandSpecItem::Cmd(CmdShape {
                args: mitex_spec::ArgShape::Right {
                    pattern: ArgPattern::Greedy,
                },
                alias: None,
            }),
        );
    }

    // Spacing commands (no args)
    for name in [",", ";", ":", ">", "!", "quad", "qquad"] {
        b.add_command(name, TEX_SYMBOL.clone());
    }

    b.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_variable() {
        let result = latex_to_omml("x", false).unwrap();
        assert!(result.contains("<m:oMath>"));
        assert!(result.contains("<m:r><m:rPr><m:sty m:val=\"i\"/></m:rPr><m:t>x</m:t></m:r>"));
        assert!(result.contains("</m:oMath>"));
        assert!(!result.contains("oMathPara"));
    }

    #[test]
    fn test_display_math() {
        let result = latex_to_omml("x", true).unwrap();
        assert!(result.contains("<m:oMathPara><m:oMath>"));
        assert!(result.contains("</m:oMath></m:oMathPara>"));
    }

    #[test]
    fn test_superscript() {
        let result = latex_to_omml("x^2", false).unwrap();
        assert!(result.contains("<m:sSup>"));
        assert!(result.contains("<m:e>"));
        assert!(result.contains("<m:sup>"));
        assert!(result.contains("</m:sSup>"));
    }

    #[test]
    fn test_subscript() {
        let result = latex_to_omml("x_i", false).unwrap();
        assert!(result.contains("<m:sSub>"));
        assert!(result.contains("<m:sub>"));
    }

    #[test]
    fn test_fraction() {
        let result = latex_to_omml("\\frac{a}{b}", false).unwrap();
        assert!(result.contains("<m:f>"));
        assert!(result.contains("<m:num>"));
        assert!(result.contains("<m:den>"));
    }

    #[test]
    fn test_sqrt() {
        let result = latex_to_omml("\\sqrt{x}", false).unwrap();
        assert!(result.contains("<m:rad>"));
        assert!(result.contains("<m:degHide m:val=\"1\""));
    }

    #[test]
    fn test_sqrt_with_degree() {
        let result = latex_to_omml("\\sqrt[3]{x}", false).unwrap();
        assert!(result.contains("<m:rad>"));
        assert!(result.contains("<m:degHide m:val=\"0\""));
        assert!(result.contains("<m:deg>"));
    }

    #[test]
    fn test_sum_with_limits() {
        let result = latex_to_omml("\\sum_{i=0}^{n}", false).unwrap();
        assert!(result.contains("<m:nary>"));
        assert!(result.contains("\u{2211}")); // ∑
        assert!(result.contains("<m:sub>"));
        assert!(result.contains("<m:sup>"));
    }

    #[test]
    fn test_greek_letter() {
        let result = latex_to_omml("\\alpha", false).unwrap();
        assert!(result.contains("\u{03B1}")); // α
    }

    #[test]
    fn test_matrix() {
        let result = latex_to_omml("\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}", false).unwrap();
        assert!(result.contains("<m:d>")); // delimiters
        assert!(result.contains("<m:m>")); // matrix
        assert!(result.contains("<m:mr>")); // matrix row
    }

    #[test]
    fn test_hat_accent() {
        let result = latex_to_omml("\\hat{x}", false).unwrap();
        assert!(result.contains("<m:acc>"));
        assert!(result.contains("<m:chr"));
    }

    #[test]
    fn test_numbers_not_italic() {
        let result = latex_to_omml("42", false).unwrap();
        // Numbers should not have italic style
        assert!(result.contains("<m:r><m:t>4</m:t></m:r>"));
        assert!(result.contains("<m:r><m:t>2</m:t></m:r>"));
    }

    #[test]
    fn test_complex_expression() {
        // E = mc^2
        let result = latex_to_omml("E = mc^2", false).unwrap();
        assert!(result.contains("<m:oMath>"));
        assert!(result.contains("</m:oMath>"));
    }

    #[test]
    fn test_text_command() {
        let result = latex_to_omml("\\text{hello}", false).unwrap();
        assert!(result.contains("<m:sty m:val=\"nor\""));
        assert!(result.contains("hello"));
    }

    #[test]
    fn test_hat_beta() {
        let result = latex_to_omml("\\hat{\\beta}", false).unwrap();
        assert!(result.contains("<m:acc>"), "should have accent: {result}");
        assert!(result.contains("\u{03B2}"), "should have beta: {result}");
    }

    #[test]
    fn test_r_squared_formula() {
        let latex = r"R^2 = 1 - \frac{\sum_{i=1}^{n}(y_i - \hat{y}_i)^2}{\sum_{i=1}^{n}(y_i - \bar{y})^2}";
        let result = latex_to_omml(latex, false).unwrap();
        assert!(result.contains("<m:f>"), "should have fraction: {result}");
        assert!(result.contains("<m:acc>"), "should have accent (hat/bar): {result}");
        assert!(result.contains("<m:nary>"), "should have nary (sum): {result}");
        // ( must be present
        assert!(result.contains("("), "should have open paren: {result}");
    }

    #[test]
    fn test_percent_literal() {
        let result = latex_to_omml("50\\%", false).unwrap();
        // \% should output just "%" without backslash
        assert!(result.contains("<m:t>%</m:t>"), "got: {result}");
        assert!(!result.contains("\\%"), "backslash should be removed: {result}");
    }
}
