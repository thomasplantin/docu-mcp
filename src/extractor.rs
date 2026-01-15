use std::path::Path;
use anyhow::Result;

use crate::extractors::pdf_extractor::PdfExtractor;

/// Trait for extracting text from various document formats
pub trait DocumentExtractor {
    /// Extracts text content from a file at the given path
    ///
    /// # Arguments
    /// * `file_path` - Path to the document file
    ///
    /// # Returns
    /// * `Ok(String)` - Extracted text content
    /// * `Err` - Error if extraction fails (file not found, invalid format, etc.)
    fn extract_text_from_file(&self, file_path: &Path) -> Result<String>;

    /// Returns the name/type of this extractor (e.g., "PdfExtractor", "DocxExtractor")
    fn extractor_type(&self) -> &'static str;
}

/// Creates an appropriate document extractor based on the file extension
///
/// # Arguments
/// * `file_path` - Path to the document file
///
/// # Returns
/// * `Ok(Box<dyn DocumentExtractor>)` - Appropriate extractor for the file type
/// * `Err` - Error if the file format is not supported
///
/// # Supported Formats
/// * `.pdf` - PDF documents (Phase 1)
pub fn create_extractor(file_path: &Path) -> Result<Box<dyn DocumentExtractor>> {
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| anyhow::anyhow!("File has no extension: {}", file_path.display()))?;

    match extension.to_lowercase().as_str() {
        "pdf" => Ok(Box::new(PdfExtractor)),
        _ => Err(anyhow::anyhow!(
            "Unsupported file format: {}. Only PDF files are supported in Phase 1.",
            extension
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_create_extractor_for_pdf() {
        // Get the path to the test PDF
        let mut pdf_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pdf_path.push("fixtures");
        pdf_path.push("boardingPass.pdf");

        // Test factory function with PDF
        let result = create_extractor(&pdf_path);
        assert!(result.is_ok(), "Factory should create extractor for PDF files");
        
        let extractor = result.unwrap();
        
        // Verify that the extractor is indeed PdfExtractor
        assert_eq!(
            extractor.extractor_type(),
            "PdfExtractor",
            "Factory should return PdfExtractor instance for PDF files"
        );
        
        // Test that the extractor actually works
        let text_result = extractor.extract_text_from_file(&pdf_path);
        assert!(text_result.is_ok(), "Extractor should extract text from PDF");
        assert!(!text_result.unwrap().is_empty(), "Extracted text should not be empty");
    }

    #[test]
    fn test_create_extractor_for_unsupported_format() {
        let mut txt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        txt_path.push("fixtures");
        txt_path.push("test.txt");

        // Test factory function with unsupported format
        let result = create_extractor(&txt_path);
        assert!(result.is_err(), "Factory should return error for unsupported formats");
        
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(error_msg.contains("Unsupported file format"), 
                    "Error message should mention unsupported format. Got: {}", error_msg);
        }
    }

    #[test]
    fn test_create_extractor_for_file_without_extension() {
        let path = PathBuf::from("somefile");

        // Test factory function with file without extension
        let result = create_extractor(&path);
        assert!(result.is_err(), "Factory should return error for files without extension");
    }
}
