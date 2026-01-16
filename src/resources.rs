use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use crate::config::load_config;
use crate::extractor::create_extractor;
use crate::constants::{SUPPORTED_FILE_EXTENSIONS, get_mime_type};

/// MCP Resource structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// Resource URI (e.g., "pdf://filename.pdf", "docx://document.docx")
    pub uri: String,
    /// Resource name (filename)
    pub name: String,
    /// Resource description
    pub description: Option<String>,
    /// MIME type
    pub mime_type: Option<String>,
}

/// Resource content structure
#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceContent {
    /// Resource URI
    pub uri: String,
    /// Extracted text content
    pub text: String,
    /// MIME type
    pub mime_type: Option<String>,
}

/// Parse a resource URI to extract the filename
///
/// # Arguments
/// * `uri` - Resource URI (e.g., "pdf://filename.pdf", "docx://document.docx")
///
/// # Returns
/// * `Ok(String)` - Extracted filename
/// * `Err` - Error if URI format is invalid
fn parse_resource_uri(uri: &str) -> Result<String> {
    // Check if URI starts with any supported scheme (extension + "://")
    let accepted_scheme = SUPPORTED_FILE_EXTENSIONS
        .iter()
        .find_map(|ext| {
            let scheme = format!("{}://", ext);
            uri.starts_with(&scheme).then_some(scheme)
        })
        .ok_or_else(|| {
            let schemes_list: Vec<String> = SUPPORTED_FILE_EXTENSIONS
                .iter()
                .map(|ext| format!("{}://", ext))
                .collect();
            anyhow::anyhow!("Invalid URI format. Expected one of: {}, got: {}", schemes_list.join(", "), uri)
        })?;
    
    let filename = uri.strip_prefix(&accepted_scheme)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse URI: {}", uri))?;
    
    if filename.is_empty() {
        return Err(anyhow::anyhow!("URI contains no filename: {}", uri));
    }
    
    Ok(filename.to_string())
}

/// Get the full file path for a resource URI
///
/// # Arguments
/// * `uri` - Resource URI (e.g., "pdf://filename.pdf", "docx://document.docx")
///
/// # Returns
/// * `Ok(PathBuf)` - Full path to the file
/// * `Err` - Error if URI is invalid or file doesn't exist in active directory
fn get_resource_path(uri: &str) -> Result<PathBuf> {
    let filename = parse_resource_uri(uri)?;
    
    // Get active directory from config
    let config = load_config()?;
    let active_dir = config.active_directory
        .ok_or_else(|| anyhow::anyhow!("No active directory set. Use set_document_directory tool first."))?;
    
    let active_path = Path::new(&active_dir);
    let file_path = active_path.join(&filename);
    
    // Validate file exists in active directory
    if !file_path.exists() {
        return Err(anyhow::anyhow!(
            "File not found in active directory: {}. Active directory: {}",
            filename,
            active_dir
        ));
    }
    
    // Validate it's actually in the active directory (prevent directory traversal)
    let canonical_file = file_path.canonicalize()
        .with_context(|| format!("Failed to canonicalize file path: {}", file_path.display()))?;
    let canonical_dir = active_path.canonicalize()
        .with_context(|| format!("Failed to canonicalize directory path: {}", active_dir))?;
    
    if !canonical_file.starts_with(&canonical_dir) {
        return Err(anyhow::anyhow!(
            "File is not in active directory (security check failed): {}",
            filename
        ));
    }
    
    Ok(canonical_file)
}

/// List all resources in the active directory
///
/// # Returns
/// * `Ok(Vec<Resource>)` - List of resources with supported file extensions
/// * `Err` - Error if active directory is not set or cannot be read
pub fn list_resources() -> Result<Vec<Resource>> {
    let config = load_config()?;
    let active_dir = config.active_directory
        .ok_or_else(|| anyhow::anyhow!("No active directory set. Use set_document_directory tool first."))?;
    
    let active_path = Path::new(&active_dir);
    
    // Validate directory exists and is readable
    if !active_path.exists() {
        return Err(anyhow::anyhow!("Active directory does not exist: {}", active_dir));
    }
    
    let entries = std::fs::read_dir(active_path)
        .with_context(|| format!("Failed to read active directory: {}", active_dir))?;
    
    let mut resources = Vec::new();
    
    for entry in entries {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();
        
        // Skip if not a file
        if !path.is_file() {
            continue;
        }
        
        // Skip if no extension
        let extension = match path.extension() {
            Some(ext) => ext,
            None => continue,
        };
        
        // Skip if extension is not in supported list
        let extension_str = match extension.to_str() {
            Some(ext) => ext.to_lowercase(),
            None => continue,
        };
        
        if !SUPPORTED_FILE_EXTENSIONS.contains(&extension_str.as_str()) {
            continue;
        }
        
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid filename: {}", path.display()))?;
        
        // Construct URI scheme from file extension
        let uri = format!("{}://{}", extension_str, filename);
        
        // Determine MIME type based on extension
        let mime_type = get_mime_type(&extension_str);
        
        resources.push(Resource {
            uri,
            name: filename.to_string(),
            description: Some(format!("Document: {}", filename)),
            mime_type: Some(mime_type.to_string()),
        });
    }
    
    Ok(resources)
}

/// Get content for a resource URI
///
/// # Arguments
/// * `uri` - Resource URI (e.g., "pdf://filename.pdf", "docx://document.docx")
///
/// # Returns
/// * `Ok(ResourceContent)` - Resource content with extracted text
/// * `Err` - Error if URI is invalid, file doesn't exist, or extraction fails
pub fn get_resource(uri: &str) -> Result<ResourceContent> {
    let file_path = get_resource_path(uri)?;
    
    // Create appropriate extractor
    let extractor = create_extractor(&file_path)
        .with_context(|| format!("Failed to create extractor for resource: {}", uri))?;
    
    // Extract text
    let text = extractor.extract_text_from_file(&file_path)
        .with_context(|| format!("Failed to extract text from resource: {}", uri))?;
    
    // Determine MIME type based on file extension
    let mime_type = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| get_mime_type(ext).to_string());
    
    Ok(ResourceContent {
        uri: uri.to_string(),
        text,
        mime_type,
    })
}
