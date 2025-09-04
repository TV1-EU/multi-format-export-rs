# multi-format-export-rs

A small Rust library that renders Handlebars templates to Markdown and exports the resulting Markdown into multiple formats:
- Markdown (pass-through)
- HTML (via `markdown`)
- PDF (via Typst)
- DOCX (via `docx-rs`)

## Features
- Plug-in style exporters behind a simple trait
- Handlebars template rendering
- Basic Markdown AST → DOCX conversion with font customization
- Lightweight Markdown → Typst → PDF pipeline (embeds default fonts)
- Easily extensible to add new formats

## Example

```rust
use multi_format_export_rs::{
    multi_format_export_engine::{MultiFormatExportEngine, OutputFormat},
};
use serde::Serialize;

#[derive(Serialize)]
struct Context {
    title: String,
    items: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = MultiFormatExportEngine::new();

    // Register a Handlebars template (name: "list")
    engine.register_template_string("list", r#"
# {{title}}

{{#each items}}
- {{this}}
{{/each}}
"#)?;

    // Render template to Markdown
    let md = engine.render("list", &Context {
        title: "Shopping List".into(),
        items: vec!["Apples".into(), "Bread".into(), "Tea".into()],
    })?;

    // Export to PDF (similarly: Md, Html, Docx)
    let exported = engine.convert(&md, &OutputFormat::Pdf)?;
    std::fs::write(format!("output.{}", exported.extension), &exported.data)?;

    Ok(())
}
```

Another example can be found in the `examples` directory. Run it with `cargo run --example basic`.

## Adding via Cargo (git)

Since this crate is not published on crates.io, add it directly from the repository:

```toml
[dependencies]
multi-format-export-rs = { git = "https://github.com/TV1-EU/multi-format-export-rs" }
```

## Extending

Implement the `Export` trait and register your exporter in `MultiFormatExportEngine::new()` (or expose a registration method) to support additional formats (e.g. EPUB).

## License

MIT

## Status

Early-stage; PDF and DOCX mappings are intentionally minimal. Contributions welcome.
