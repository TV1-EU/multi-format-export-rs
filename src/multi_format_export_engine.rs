use std::{collections::HashMap, str::FromStr};

use handlebars::Handlebars;
use serde::{Deserialize, Serialize};

use crate::{
    error::MultiFormatExportError,
    exporter::{
        Export, Exported, docx::DocxExporter, html::HtmlExporter, markdown::MarkdownExporter,
        pdf::PdfExporter,
    },
};

pub struct MultiFormatExportEngine {
    handlebars: Handlebars<'static>,
    exporters: HashMap<OutputFormat, Box<dyn Export>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Md,
    Html,
    Pdf,
    Docx,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Md => write!(f, "md"),
            OutputFormat::Html => write!(f, "html"),
            OutputFormat::Pdf => write!(f, "pdf"),
            OutputFormat::Docx => write!(f, "docx"),
        }
    }
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "md" | "markdown" => Ok(OutputFormat::Md),
            "html" => Ok(OutputFormat::Html),
            "pdf" => Ok(OutputFormat::Pdf),
            "docx" => Ok(OutputFormat::Docx),
            _ => Err(format!("Invalid output format: {}", s)),
        }
    }
}

impl MultiFormatExportEngine {
    pub fn new() -> Self {
        let handlebars = Handlebars::new();
        let mut exporters = HashMap::<OutputFormat, Box<dyn Export>>::new();

        exporters.insert(OutputFormat::Html, Box::new(HtmlExporter::new()));
        exporters.insert(OutputFormat::Pdf, Box::new(PdfExporter::default()));
        exporters.insert(OutputFormat::Docx, Box::new(DocxExporter::default()));
        exporters.insert(OutputFormat::Md, Box::new(MarkdownExporter::new()));

        Self {
            handlebars,
            exporters,
        }
    }

    pub fn register_template_string(
        &mut self,
        name: &str,
        tpl: &str,
    ) -> Result<(), MultiFormatExportError> {
        self.handlebars.register_template_string(name, tpl)?;
        Ok(())
    }

    pub fn render<T: Serialize>(
        &self,
        name: &str,
        data: &T,
    ) -> Result<String, MultiFormatExportError> {
        self.handlebars
            .render(name, data)
            .map_err(|e| MultiFormatExportError::RenderError(e))
    }

    pub fn supported_formats(&self) -> Vec<OutputFormat> {
        vec![
            OutputFormat::Md,
            OutputFormat::Html,
            OutputFormat::Pdf,
            OutputFormat::Docx,
        ]
    }

    pub fn convert(
        &self,
        template_str: &str,
        format: &OutputFormat,
    ) -> Result<Exported, MultiFormatExportError> {
        let exporter = self
            .exporters
            .get(format)
            .ok_or(MultiFormatExportError::UnsupportedFormat(format.clone()))?;

        exporter.export(template_str)
    }
}
