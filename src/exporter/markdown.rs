use crate::{
    error::MultiFormatExportError,
    exporter::{Export, Exported},
};

const MARKDOWN_MIME: &'static str = "text/markdown";
const MARKDOWN_EXTENSION: &'static str = "md";

pub struct MarkdownExporter;

impl MarkdownExporter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Export for MarkdownExporter {
    fn export(&self, content: &str) -> Result<Exported, MultiFormatExportError> {
        Ok(Exported {
            data: content.to_string().into(),
            mime: MARKDOWN_MIME,
            extension: MARKDOWN_EXTENSION,
        })
    }
}
