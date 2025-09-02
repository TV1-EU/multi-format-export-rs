use crate::{
    error::MultiFormatExportError,
    exporter::{Export, Exported},
};

pub struct HtmlExporter;

const HTML_EXTENSION: &'static str = "html";
const HTML_MIME: &'static str = "text/html";

impl HtmlExporter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Export for HtmlExporter {
    fn export(&self, content: &str) -> Result<Exported, MultiFormatExportError> {
        Ok(Exported {
            data: markdown::to_html(content).into(),
            mime: HTML_MIME,
            extension: HTML_EXTENSION,
        })
    }
}
