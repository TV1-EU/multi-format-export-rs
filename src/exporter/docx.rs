use std::io::Cursor;

use bytes::Bytes;
use derive_new::new;
use docx_rs::{
    BreakType, Docx, Paragraph as DocxParagraph, Run as DocxRun, RunFonts, SpecialIndentType,
};
use markdown::{ParseOptions, mdast, mdast::Node};

use crate::{
    error::MultiFormatExportError,
    exporter::{Export, Exported},
};

#[derive(new)]
pub struct DocxExporter {
    default_font_family: String, // e.g. "Times New Roman"
    mono_font_family: String,    // e.g. "Courier New"
    default_font_size: usize,    // half-points (22 = 11pt)
}

const DOCX_MIME: &'static str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const DOCX_EXTENSION: &'static str = "docx";

// Indentation (in twips: 1440 twips = 1 inch)
const LIST_BASE_LEFT: i32 = 720; // 0.5"
const LIST_LEVEL_INCREMENT: i32 = 360; // 0.25"
const LIST_HANGING: i32 = 360; // Hanging indent for bullet/number

impl Default for DocxExporter {
    fn default() -> Self {
        Self {
            default_font_family: "Times New Roman".to_string(),
            mono_font_family: "Courier New".to_string(),
            default_font_size: 22, // 11pt
        }
    }
}

impl DocxExporter {
    // ---------------- Headings ----------------

    // Map heading depth (1..=6) to half-point font sizes (Word uses half-points: 32 = 16pt)
    fn heading_font_size(&self, depth: usize) -> usize {
        let body_half = self.default_font_size as f32;
        let multiplier = match depth {
            1 => 1.60, // roughly 176% of body
            2 => 1.45,
            3 => 1.30,
            4 => 1.15,
            5 => 1.05,
            _ => 1.00,
        };
        let mut hp = (body_half * multiplier).round() as isize;
        if hp < 2 {
            hp = 2;
        } // avoid zero / invalid
        if hp > 400 {
            hp = 400;
        } // ~200pt
        hp as usize
    }

    // Scale spacing relative to an 11pt baseline (original static values assumed 11pt body).
    // Original baseline (before, after) for depths: (360,180),(320,160),(300,140),(240,120)...
    fn heading_spacing(&self, depth: usize) -> (u32, u32) {
        let (base_before, base_after) = match depth {
            1 => (360u32, 180u32),
            2 => (320, 160),
            3 => (300, 140),
            _ => (240, 120),
        };
        let body_pt = self.default_font_size as f32 / 2.0;
        let ratio = body_pt / 11.0; // 11pt was the original implicit baseline
        let scale = |v: u32| -> u32 {
            let scaled = (v as f32 * ratio).round();
            // Ensure we don't collapse to zero if body gets very small
            scaled.max(20.0) as u32
        };
        (scale(base_before), scale(base_after))
    }

    fn render_heading_node(&self, heading: &mdast::Heading) -> DocxParagraph {
        let depth = heading.depth as usize;
        let size = self.heading_font_size(depth);
        let (before, after) = self.heading_spacing(depth);

        let mut p = DocxParagraph::new()
            .line_spacing(docx_rs::LineSpacing::new().before(before).after(after));

        // Inline children -> all runs with base heading size
        p = self.append_inline_children_with_base(p, &heading.children, true, false, size, false);
        p
    }

    // Pattern: bold first line treated as heading2
    fn is_strong_line_heading(&self, p: &mdast::Paragraph) -> bool {
        if p.children.is_empty() {
            return false;
        }
        if p.children.len() == 1 {
            return matches!(p.children[0], Node::Strong(_));
        }
        if p.children.len() == 2 {
            if let Node::Strong(_) = p.children[0] {
                if let Node::Text(t) = &p.children[1] {
                    return t.value.starts_with('\n');
                }
            }
        }
        false
    }

    fn split_paragraph_heading(
        &self,
        p: &mdast::Paragraph,
    ) -> Option<(DocxParagraph, Option<DocxParagraph>)> {
        if !self.is_strong_line_heading(p) {
            return None;
        }
        // Treat as level 2 heading
        let size = self.heading_font_size(2);
        let (before, after) = self.heading_spacing(2);
        let mut heading_para = DocxParagraph::new()
            .line_spacing(docx_rs::LineSpacing::new().before(before).after(after));

        heading_para = self.append_inline_children_with_base(
            heading_para,
            &p.children[0..1],
            true,
            false,
            size,
            false,
        );

        if p.children.len() == 1 {
            return Some((heading_para, None));
        }

        if let Node::Text(t) = &p.children[1] {
            let rest = t.value.trim_start_matches('\n');
            if rest.is_empty() {
                return Some((heading_para, None));
            }
            let mut body_para = self.new_body_paragraph();
            let remainder_node = Node::Text(mdast::Text {
                value: rest.to_string(),
                position: None,
            });
            body_para = self.append_inline_children_with_base(
                body_para,
                std::slice::from_ref(&remainder_node),
                false,
                false,
                0,
                false,
            );
            Some((heading_para, Some(body_para)))
        } else {
            Some((heading_para, None))
        }
    }

    // --------------- Lists with real indentation (not spaces) ---------------

    fn list_left_indent(depth: usize) -> i32 {
        LIST_BASE_LEFT + (depth as i32) * LIST_LEVEL_INCREMENT
    }

    fn render_list(&self, list: &mdast::List, depth: usize) -> Vec<DocxParagraph> {
        let mut out = Vec::new();
        let mut index = list.start.unwrap_or(1);

        for item_node in &list.children {
            let Node::ListItem(item) = item_node else {
                continue;
            };
            let mut first_block = true;

            for child in &item.children {
                match child {
                    Node::Paragraph(p) => {
                        let mut para = DocxParagraph::new().indent(
                            Some(Self::list_left_indent(depth)),
                            Some(SpecialIndentType::Hanging(LIST_HANGING)),
                            None,
                            None,
                        );

                        if first_block {
                            let marker = if list.ordered {
                                format!("{}.", index)
                            } else {
                                "•".to_string()
                            };
                            para = para.add_run(DocxRun::new().bold().add_text(marker + " "));
                        } else {
                            para = DocxParagraph::new().indent(
                                Some(Self::list_left_indent(depth) + LIST_HANGING),
                                None,
                                None,
                                None,
                            );
                        }

                        para = self.append_inline_children_with_base(
                            para,
                            &p.children,
                            false,
                            false,
                            0,
                            false,
                        );
                        out.push(para);
                        first_block = false;
                    }
                    Node::List(nested) => {
                        let nested_vec = self.render_list(nested, depth + 1);
                        out.extend(nested_vec);
                    }
                    other => {
                        let blocks = self.render_block_node(other, depth + 1);
                        out.extend(blocks);
                    }
                }
            }

            if list.ordered {
                index += 1;
            }
        }

        out
    }

    // ---------------- Block dispatcher ----------------

    fn render_block_node(&self, node: &Node, depth: usize) -> Vec<DocxParagraph> {
        match node {
            Node::Paragraph(p) => {
                if let Some((heading, rest)) = self.split_paragraph_heading(p) {
                    let mut v = vec![heading];
                    if let Some(r) = rest {
                        v.push(r);
                    }
                    v
                } else {
                    vec![self.render_paragraph(p)]
                }
            }
            Node::Heading(h) => vec![self.render_heading_node(h)],
            Node::Code(code_block) => vec![self.render_code_block(code_block)],
            Node::List(list) => self.render_list(list, depth),
            Node::Text(_)
            | Node::Strong(_)
            | Node::Emphasis(_)
            | Node::Break(_)
            | Node::InlineCode(_) => {
                let mut para = self.new_body_paragraph();
                para = self.append_inline_children_with_base(
                    para,
                    std::slice::from_ref(node),
                    false,
                    false,
                    0,
                    false,
                );
                vec![para]
            }
            _ => Vec::new(),
        }
    }

    fn render_paragraph(&self, p: &mdast::Paragraph) -> DocxParagraph {
        let mut para = self.new_body_paragraph();
        para = self.append_inline_children_with_base(para, &p.children, false, false, 0, false);
        para
    }

    fn render_code_block(&self, code: &mdast::Code) -> DocxParagraph {
        let mut p = self.new_body_paragraph();
        p = p.indent(Some(0), None, None, None);

        // Split code by newlines and create runs with breaks
        for (i, line) in code.value.lines().enumerate() {
            let mut run = DocxRun::new()
                .fonts(
                    RunFonts::new()
                        .ascii(&self.mono_font_family)
                        .hi_ansi(&self.mono_font_family),
                )
                .add_text(line.to_string());

            if self.default_font_size > 0 {
                run = run.size(self.default_font_size);
            }

            p = p.add_run(run);

            // Add line break after each line except the last
            if i < code.value.lines().count() - 1 {
                let break_run = DocxRun::new().add_break(BreakType::TextWrapping);
                p = p.add_run(break_run);
            }
        }

        p
    }

    // ---------------- Inline handling ----------------

    fn append_inline_children_with_base(
        &self,
        mut paragraph: DocxParagraph,
        nodes: &[Node],
        force_bold: bool,
        force_italic: bool,
        base_size: usize,
        mono: bool,
    ) -> DocxParagraph {
        for node in nodes {
            match node {
                Node::Text(t) => {
                    let mut parts = t.value.split('\n').peekable();
                    while let Some(part) = parts.next() {
                        if !part.is_empty() {
                            paragraph = self.add_text_run(
                                paragraph,
                                part,
                                force_bold,
                                force_italic,
                                mono,
                                base_size,
                            );
                        }
                        if parts.peek().is_some() {
                            paragraph = paragraph
                                .add_run(DocxRun::new().add_break(BreakType::TextWrapping));
                        }
                    }
                }
                Node::InlineCode(ic) => {
                    paragraph = self.add_text_run(
                        paragraph,
                        &ic.value,
                        force_bold,
                        force_italic,
                        true,
                        base_size,
                    );
                }
                Node::Code(c) => {
                    paragraph = self.add_text_run(
                        paragraph,
                        &c.value,
                        force_bold,
                        force_italic,
                        true,
                        base_size,
                    );
                }
                Node::Emphasis(em) => {
                    paragraph = self.append_inline_children_with_base(
                        paragraph,
                        &em.children,
                        force_bold,
                        true,
                        base_size,
                        mono,
                    );
                }
                Node::Strong(st) => {
                    paragraph = self.append_inline_children_with_base(
                        paragraph,
                        &st.children,
                        true,
                        force_italic,
                        base_size,
                        mono,
                    );
                }
                Node::Break(_) => {
                    paragraph =
                        paragraph.add_run(DocxRun::new().add_break(BreakType::TextWrapping));
                }
                other => {
                    let txt = self.collect_plain_text(std::slice::from_ref(other));
                    if !txt.is_empty() {
                        paragraph = self.add_text_run(
                            paragraph,
                            &txt,
                            force_bold,
                            force_italic,
                            mono,
                            base_size,
                        );
                    }
                }
            }
        }
        paragraph
    }

    fn add_text_run(
        &self,
        paragraph: DocxParagraph,
        text: &str,
        bold: bool,
        italic: bool,
        mono: bool,
        size: usize,
    ) -> DocxParagraph {
        if text.is_empty() {
            return paragraph;
        }
        let mut run = DocxRun::new().add_text(text.to_string());

        if bold {
            run = run.bold();
        }
        if italic {
            run = run.italic();
        }

        if mono {
            run = run.fonts(
                RunFonts::new()
                    .ascii(&self.mono_font_family)
                    .hi_ansi(&self.mono_font_family),
            );
        } else {
            // Apply body font
            run = run.fonts(
                RunFonts::new()
                    .ascii(&self.default_font_family)
                    .hi_ansi(&self.default_font_family),
            );
        }

        // size > 0 means a specific caller (e.g., heading) provided size.
        // Otherwise use default body size.
        let effective_size = if size > 0 {
            size
        } else {
            self.default_font_size
        };
        if effective_size > 0 {
            run = run.size(effective_size);
        }

        paragraph.add_run(run)
    }

    fn collect_plain_text(&self, nodes: &[Node]) -> String {
        let mut buf = String::new();
        for n in nodes {
            match n {
                Node::Text(t) => buf.push_str(&t.value),
                Node::InlineCode(ic) => buf.push_str(&ic.value),
                Node::Break(_) => buf.push('\n'),
                _ => {}
            }
        }
        buf
    }

    fn body_paragraph_spacing(&self) -> (u32, u32) {
        // baseline: before = 0, after = 160 twips (~8pt)
        let base_before = 0u32;
        let base_after = 160u32;

        let body_pt = self.default_font_size as f32 / 2.0;
        let ratio = body_pt / 11.0;
        let scale = |v: u32| -> u32 {
            if v == 0 {
                0
            } else {
                let scaled = (v as f32 * ratio).round();
                // avoid collapsing to 0 if scaling gets very small
                scaled.max(20.0) as u32
            }
        };
        (scale(base_before), scale(base_after))
    }

    fn new_body_paragraph(&self) -> DocxParagraph {
        let (before, after) = self.body_paragraph_spacing();
        DocxParagraph::new().line_spacing(docx_rs::LineSpacing::new().before(before).after(after))
    }
}

impl Export for DocxExporter {
    fn export(&self, content: &str) -> Result<Exported, MultiFormatExportError> {
        let md_ast = markdown::to_mdast(content, &ParseOptions::default())?;
        let mut docx = Docx::new();

        if let Some(children) = md_ast.children() {
            for node in children {
                for para in self.render_block_node(node, 0) {
                    docx = docx.add_paragraph(para);
                }
            }
        }

        let mut cursor = Cursor::new(Vec::new());
        docx.build()
            .pack(&mut cursor)
            .map_err(|err| MultiFormatExportError::DocxError(err.to_string()))?;
        let bytes = Bytes::from(cursor.into_inner());

        Ok(Exported {
            data: bytes,
            mime: DOCX_MIME,
            extension: DOCX_EXTENSION,
        })
    }
}
