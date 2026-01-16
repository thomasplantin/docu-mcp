/// File extension constants
pub const PDF_EXTENSION: &str = "pdf";
pub const DOCX_EXTENSION: &str = "docx";
pub const DOC_EXTENSION: &str = "doc";
pub const TXT_EXTENSION: &str = "txt";

/// Supported file extensions for document extraction
/// 
/// These extensions define which file types can be processed and listed as resources.
/// Currently only PDF is supported, but this can be extended in the future.
pub const SUPPORTED_FILE_EXTENSIONS: &[&str] = &[PDF_EXTENSION];

/// Get MIME type for a given file extension
/// 
/// # Arguments
/// * `extension` - File extension (case-insensitive)
/// 
/// # Returns
/// MIME type string, or "application/octet-stream" if extension is not recognized
pub fn get_mime_type(extension: &str) -> &'static str {
    match extension.to_lowercase().as_str() {
        PDF_EXTENSION => "application/pdf",
        DOCX_EXTENSION => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        DOC_EXTENSION => "application/msword",
        TXT_EXTENSION => "text/plain",
        _ => "application/octet-stream",
    }
}

