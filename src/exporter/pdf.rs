use std::borrow::Cow;

use crate::{
    error::MultiFormatExportError,
    exporter::{Export, Exported},
};
use bytes::Bytes;
use markdown::{ParseOptions, mdast};
use typst_as_lib::TypstEngine;
use typst_pdf::PdfOptions;

const PDF_MIME: &'static str = "application/pdf";
const PDF_EXTENSION: &'static str = "pdf";
const DEFAULT_TEMPLATE: &str = r#"
#set page(paper: "a4")
#set text(font: "Liberation Serif", 11pt)


{{content}}
"#;

/// A simple Typst-based PDF exporter.
/// Template must contain the placeholder `{{content}}`.
pub struct PdfExporter {
    template: String,
    fonts: Vec<&'static [u8]>,
}

impl Default for PdfExporter {
    fn default() -> Self {
        Self::new(None, &[])
    }
}

impl PdfExporter {
    /// Create a new PdfExporter.
    /// - template: Optional template string. If None, a default is used.
    /// - fonts: Optional slice of font byte slices (static). If empty, Typst's defaults / embedded fonts are used.
    pub fn new<T: Into<Option<String>>>(template: T, fonts: &[&'static [u8]]) -> Self {
        let tmpl = template
            .into()
            .unwrap_or_else(|| DEFAULT_TEMPLATE.to_string());

        let mut fonts = fonts.to_vec();
        if fonts.is_empty() {
            fonts.push(include_bytes!("../../assets/fonts/NotoSans-Bold.ttf"));
            fonts.push(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf"));
        }

        Self {
            template: tmpl,
            fonts: fonts.to_vec(),
        }
    }

    /// Very lightweight markdown→Typst conversion.
    /// Extend as needed (images, links, tables, etc.).
    fn md_to_typst(&self, node: &mdast::Node) -> String {
        let mut out = String::new();
        if let Some(children) = node.children() {
            for child in children {
                out.push_str(&self.render_block(child));
            }
        }
        out
    }

    fn render_block(&self, node: &mdast::Node) -> String {
        match node {
            mdast::Node::Heading(h) => {
                let txt = self.collect_inlines(&h.children);
                let eqs = "=".repeat(h.depth as usize);
                format!("\n{eqs} {txt}\n\n")
            }
            mdast::Node::Paragraph(p) => {
                let txt = self.collect_inlines(&p.children);
                if txt.trim().is_empty() {
                    String::new()
                } else {
                    format!("{txt}\n\n")
                }
            }
            mdast::Node::Code(c) => {
                // Typst code block: ```language ... ```
                // (Typst currently also accepts raw fences similar to Markdown.)
                let lang = c.lang.clone().unwrap_or_default();
                if lang.is_empty() {
                    format!("```{}\n{}\n```\n\n", "", self.escape_code(&c.value))
                } else {
                    format!("```{}\n{}\n```\n\n", lang, self.escape_code(&c.value))
                }
            }
            mdast::Node::List(list) => self.render_list(list),
            // Fallback: treat stray inline nodes as a paragraph
            mdast::Node::Strong(_)
            | mdast::Node::Emphasis(_)
            | mdast::Node::InlineCode(_)
            | mdast::Node::Text(_)
            | mdast::Node::Break(_) => {
                let txt = self.collect_inlines(std::slice::from_ref(node));
                if txt.is_empty() {
                    "".to_string()
                } else {
                    format!("{txt}\n\n")
                }
            }
            _ => String::new(),
        }
    }

    fn render_list(&self, list: &mdast::List) -> String {
        let mut out = String::new();
        let mut index = list.start.unwrap_or(1);
        for item_node in &list.children {
            if let mdast::Node::ListItem(item) = item_node {
                // Concatenate all paragraph-like children into one for simple approach
                let mut item_buf = String::new();
                for c in &item.children {
                    match c {
                        mdast::Node::Paragraph(p) => {
                            item_buf.push_str(&self.collect_inlines(&p.children));
                        }
                        mdast::Node::List(nested) => {
                            // Indent nested list lines by two spaces
                            let nested_str = self.render_list(nested);
                            for line in nested_str.lines() {
                                if !line.trim().is_empty() {
                                    item_buf.push('\n');
                                    item_buf.push_str("  ");
                                    item_buf.push_str(line);
                                }
                            }
                        }
                        other => {
                            item_buf.push_str(&self.render_block(other));
                        }
                    }
                }
                if list.ordered {
                    out.push_str(&format!("{}. {}\n", index, item_buf.trim()));
                    index += 1;
                } else {
                    out.push_str(&format!("- {}\n", item_buf.trim()));
                }
            }
        }
        out.push('\n');
        out
    }

    fn collect_inlines(&self, nodes: &[mdast::Node]) -> String {
        let mut buf = String::new();
        for n in nodes {
            match n {
                mdast::Node::Text(t) => buf.push_str(&self.escape_text(&t.value)),
                mdast::Node::InlineCode(ic) => {
                    buf.push('`');
                    buf.push_str(&self.escape_code(&ic.value));
                    buf.push('`');
                }
                mdast::Node::Code(c) => {
                    buf.push('`');
                    buf.push_str(&self.escape_code(&c.value));
                    buf.push('`');
                }
                mdast::Node::Strong(s) => {
                    buf.push('*');
                    buf.push_str(&self.collect_inlines(&s.children));
                    buf.push('*');
                }
                mdast::Node::Emphasis(e) => {
                    buf.push('_');
                    buf.push_str(&self.collect_inlines(&e.children));
                    buf.push('_');
                }
                mdast::Node::Break(_) => buf.push_str(" \\\n"),
                other => {
                    // Fallback to plain text of nested children
                    if let Some(ch) = other.children() {
                        buf.push_str(&self.collect_inlines(ch));
                    }
                }
            }
        }
        buf
    }

    // Escape characters that would prematurely start Typst constructs
    fn escape_text<'a>(&self, s: &'a str) -> Cow<'a, str> {
        if s.chars().any(|c| matches!(c, '{' | '}' | '[' | ']' | '#')) {
            // Very conservative simple escaping by prefixing with backslash
            // (Typst doesn't treat backslash as universal escape like LaTeX;
            // refine if needed. For many simple texts you can leave as-is.)
            let mut out = String::with_capacity(s.len() + 8);
            for ch in s.chars() {
                if matches!(ch, '{' | '}' | '[' | ']' | '#') {
                    out.push('\\');
                }
                out.push(ch);
            }
            Cow::Owned(out)
        } else {
            Cow::Borrowed(s)
        }
    }

    fn escape_code(&self, s: &str) -> String {
        // For fenced blocks we only need to ensure we don't prematurely close fence.
        s.replace("```", "`\u{200B}``") // insert zero-width space
    }

    fn inject_content(&self, template: &str, content: &str) -> String {
        template.replacen("{{content}}", content, 1)
    }
}

impl Export for PdfExporter {
    fn export(&self, content: &str) -> Result<Exported, MultiFormatExportError> {
        // 1. Parse markdown
        let md_ast = markdown::to_mdast(content, &ParseOptions::default())
            .map_err(|e| MultiFormatExportError::PdfError(format!("Markdown parse: {e}")))?;

        // 2. Convert to Typst
        let typst_body = self.md_to_typst(&md_ast);

        // 3. Build final Typst source
        let main_source = self.inject_content(&self.template, &typst_body);

        let mut builder = TypstEngine::builder().main_file(main_source);

        if !self.fonts.is_empty() {
            builder = builder.fonts(self.fonts.clone());
        }

        let engine = builder.build();

        // 5. Compile (no extra inputs for now)
        let doc = engine
            .compile()
            .output
            .map_err(|e| MultiFormatExportError::PdfError(format!("Typst output error: {e:?}")))?;

        // 6. Render PDF
        let pdf = typst_pdf::pdf(&doc, &PdfOptions::default()).map_err(|e| {
            MultiFormatExportError::PdfError(format!("Typst PDF rendering error: {e:?}"))
        })?;

        Ok(Exported {
            data: Bytes::from(pdf),
            mime: PDF_MIME,
            extension: PDF_EXTENSION,
        })
    }
}
