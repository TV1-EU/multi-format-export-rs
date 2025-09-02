use multi_format_export_rs::{
    exporter::{
        Export, docx::DocxExporter, html::HtmlExporter, markdown::MarkdownExporter,
        pdf::PdfExporter,
    },
    multi_format_export_engine::MultiFormatExportEngine,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = MultiFormatExportEngine::new();

    // read `template.md` string
    let template = include_str!("template.md");

    engine
        .register_template_string("meeting", template)
        .expect("Bad Template");

    let raw = include_str!("data.json");
    let data: serde_json::Value = serde_json::from_str(raw)?;

    let md = engine.render("meeting", &data)?;

    let html_exporter = HtmlExporter::new();
    let html = html_exporter.export(&md)?;
    std::fs::write("out.html", html.data)?;

    let markdown_exporter = MarkdownExporter::new();
    let markdown = markdown_exporter.export(&md)?;
    std::fs::write("out.md", markdown.data)?;

    let docx_exporter =
        DocxExporter::new("Times New Roman".into(), "Arial Black".into(), 22 as usize);
    let docx = docx_exporter.export(&md)?;
    std::fs::write("out.docx", docx.data)?;

    let pdf_exporter = PdfExporter::new(None, &[]);
    let pdf = pdf_exporter.export(&md)?;
    std::fs::write("out.pdf", pdf.data)?;

    Ok(())
}
