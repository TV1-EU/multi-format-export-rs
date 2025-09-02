use bytes::Bytes;

use crate::error::MultiFormatExportError;

pub mod docx;
pub mod html;
pub mod markdown;
pub mod pdf;

#[derive(Debug)]
pub struct Exported {
    pub data: Bytes,
    pub mime: &'static str,
    pub extension: &'static str,
}

pub trait Export: Send + Sync {
    fn export(&self, content: &str) -> Result<Exported, MultiFormatExportError>;
}
