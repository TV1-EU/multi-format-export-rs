use thiserror::Error;

use crate::multi_format_export_engine::OutputFormat;

#[derive(Error, Debug)]
pub enum MultiFormatExportError {
    #[error("Template error: {0}")]
    TemplateError(#[from] handlebars::TemplateError),

    #[error("Render error: {0}")]
    RenderError(#[from] handlebars::RenderError),

    #[error("Markdown error: {0}")]
    MarkdownError(markdown::message::Message),

    #[error("Docx error: {0}")]
    DocxError(String),

    #[error("Pdf error: {0}")]
    PdfError(String),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(OutputFormat),
}

impl From<markdown::message::Message> for MultiFormatExportError {
    fn from(m: markdown::message::Message) -> Self {
        MultiFormatExportError::MarkdownError(m)
    }
}
