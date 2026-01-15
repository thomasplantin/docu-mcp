use std::path::Path;
use std::fs;
use anyhow::{Context, Result};
use extractous::Extractor;
use crate::extractor::DocumentExtractor;

/// PDF document extractor using the extractous crate
pub struct PdfExtractor;

impl DocumentExtractor for PdfExtractor {
    fn extractor_type(&self) -> &'static str {
        "PdfExtractor"
    }

    fn extract_text_from_file(&self, file_path: &Path) -> Result<String> {
        // Validate that the file exists
        if !file_path.exists() {
            return Err(anyhow::anyhow!("File not found: {}", file_path.display()));
        }

        // Validate that it's a file (not a directory)
        if !file_path.is_file() {
            return Err(anyhow::anyhow!("Path is not a file: {}", file_path.display()));
        }

        // Read the PDF file into memory
        let file_bytes = fs::read(file_path)
            .with_context(|| format!("Failed to read PDF file: {}", file_path.display()))?;

        // Create extractor instance
        let extractor = Extractor::new();

        // Extract text from PDF bytes (returns StreamReader and Metadata)
        let (mut reader, _metadata) = extractor
            .extract_bytes(&file_bytes)
            .with_context(|| format!("Failed to extract text from PDF: {}", file_path.display()))?;

        // Read all text from the StreamReader
        use std::io::Read;
        let mut text = String::new();
        reader
            .read_to_string(&mut text)
            .with_context(|| format!("Failed to read extracted text from PDF: {}", file_path.display()))?;

        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_extract_text_from_pdf() {
        // Get the path to the test PDF
        let mut pdf_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pdf_path.push("fixtures");
        pdf_path.push("boardingPass.pdf");

        // Create extractor and extract text
        let extractor = PdfExtractor;
        let result = extractor.extract_text_from_file(&pdf_path);

        // Verify extraction succeeded
        assert!(result.is_ok(), "Failed to extract text from PDF: {:?}", result.err());

        let text = result.unwrap();
        
        // Verify we got some text
        assert!(!text.is_empty(), "Extracted text should not be empty");
        assert!(text.len() > 100, "Extracted text should be substantial (got {} chars)", text.len());
        
        // Verify key information is present in the extracted text
        // Note: PDF extraction may have spacing issues, so we check for key terms separately
        assert!(text.contains("THOMAS PLANTIN"), "Should contain passenger name: THOMAS PLANTIN");
        assert!(text.contains("HUGOALBERTO FLORES"), "Should contain passenger name: HUGOALBERTO FLORES");
        assert!(text.contains("CM 716"), "Should contain flight number CM 716");
        assert!(text.contains("CM 155"), "Should contain flight number CM 155");
        assert!(text.contains("28 Aug 2025"), "Should contain date");
        assert!(text.contains("AUS"), "Should contain airport code AUS");
        assert!(text.contains("PTY"), "Should contain airport code PTY");
        assert!(text.contains("MDE"), "Should contain airport code MDE");
        assert!(text.contains("2302150885602"), "Should contain e-ticket number");
        assert!(text.contains("2302150885600"), "Should contain e-ticket number");
        assert!(text.contains("BDJVMN"), "Should contain reservation code");
        
        // Verify the text contains important sections
        assert!(text.contains("Boarding gates"), "Should contain boarding information");
        assert!(text.contains("Important Information"), "Should contain important information section");
        
        // Log summary for debugging if needed
        println!("Successfully extracted {} characters from PDF", text.len());
    }
}
