use std::path::Path;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use crate::config::{load_config, save_config};
use crate::extractor::create_extractor;

/// Tool parameter for set_document_directory
#[derive(Debug, Deserialize)]
pub struct SetDocumentDirectoryParams {
    pub directory: String,
}

/// Tool parameter for extract_text_from_file
#[derive(Debug, Deserialize)]
pub struct ExtractTextFromFileParams {
    pub file_path: String,
}

/// Tool parameter for list_files_in_directory
#[derive(Debug, Deserialize)]
pub struct ListFilesInDirectoryParams {
    /// Optional directory path. If not provided, uses the active directory.
    pub directory: Option<String>,
}

/// Tool result for set_document_directory
#[derive(Debug, Serialize)]
pub struct SetDocumentDirectoryResult {
    pub message: String,
    pub active_directory: String,
}

/// Tool result for list_document_directories
#[derive(Debug, Serialize)]
pub struct ListDocumentDirectoriesResult {
    pub directories: Vec<String>,
    pub active_directory: Option<String>,
}

/// Tool result for extract_text_from_file
#[derive(Debug, Serialize)]
pub struct ExtractTextFromFileResult {
    pub text: String,
}

/// File information structure
#[derive(Debug, Serialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub is_file: bool,
    pub extension: Option<String>,
}

/// Tool result for list_files_in_directory
#[derive(Debug, Serialize)]
pub struct ListFilesInDirectoryResult {
    pub directory: String,
    pub files: Vec<FileInfo>,
}

/// Tool 1: Set document directory
/// Validates directory exists and is readable, adds to directories list if not present,
/// sets as active_directory, and saves config.
pub fn set_document_directory(params: SetDocumentDirectoryParams) -> Result<SetDocumentDirectoryResult> {
    let directory_path = Path::new(&params.directory);
    
    // Validate directory exists
    if !directory_path.exists() {
        return Err(anyhow::anyhow!("Directory does not exist: {}", params.directory));
    }
    
    // Validate it's a directory
    if !directory_path.is_dir() {
        return Err(anyhow::anyhow!("Path is not a directory: {}", params.directory));
    }
    
    // Validate it's readable
    std::fs::read_dir(directory_path)
        .with_context(|| format!("Directory is not readable: {}", params.directory))?;
    
    // Load current config
    let mut config = load_config()?;
    
    // Add to directories list if not present
    let normalized_path = directory_path
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize path: {}", params.directory))?
        .to_string_lossy()
        .to_string();
    
    if !config.directories.contains(&normalized_path) {
        config.directories.push(normalized_path.clone());
    }
    
    // Set as active directory
    config.active_directory = Some(normalized_path.clone());
    
    // Save config
    save_config(&config)?;
    
    Ok(SetDocumentDirectoryResult {
        message: format!("Directory set as active: {}", normalized_path),
        active_directory: normalized_path,
    })
}

/// Tool 2: List document directories
/// Returns list of all directories and the active directory.
pub fn list_document_directories() -> Result<ListDocumentDirectoriesResult> {
    let config = load_config()?;
    
    Ok(ListDocumentDirectoriesResult {
        directories: config.directories.clone(),
        active_directory: config.active_directory.clone(),
    })
}

/// Tool 3: Extract text from file
/// Uses DocumentExtractor trait to extract text from a file.
pub fn extract_text_from_file(params: ExtractTextFromFileParams) -> Result<ExtractTextFromFileResult> {
    let file_path = Path::new(&params.file_path);
    
    // Validate file exists
    if !file_path.exists() {
        return Err(anyhow::anyhow!("File does not exist: {}", params.file_path));
    }
    
    // Validate it's a file
    if !file_path.is_file() {
        return Err(anyhow::anyhow!("Path is not a file: {}", params.file_path));
    }
    
    // Create appropriate extractor
    let extractor = create_extractor(file_path)
        .with_context(|| format!("Failed to create extractor for file: {}", params.file_path))?;
    
    // Extract text
    let text = extractor.extract_text_from_file(file_path)
        .with_context(|| format!("Failed to extract text from file: {}", params.file_path))?;
    
    Ok(ExtractTextFromFileResult { text })
}

/// Tool 4: List files in directory
/// Lists all files and subdirectories in the specified directory.
/// If no directory is provided, uses the active directory.
pub fn list_files_in_directory(params: ListFilesInDirectoryParams) -> Result<ListFilesInDirectoryResult> {
    let directory_path = if let Some(dir) = params.directory {
        Path::new(&dir).to_path_buf()
    } else {
        // Use active directory if not specified
        let config = load_config()?;
        let active_dir = config.active_directory
            .ok_or_else(|| anyhow::anyhow!("No active directory set. Use set_document_directory tool first, or provide a directory parameter."))?;
        Path::new(&active_dir).to_path_buf()
    };
    
    // Validate directory exists
    if !directory_path.exists() {
        return Err(anyhow::anyhow!("Directory does not exist: {}", directory_path.display()));
    }
    
    // Validate it's a directory
    if !directory_path.is_dir() {
        return Err(anyhow::anyhow!("Path is not a directory: {}", directory_path.display()));
    }
    
    // Read directory entries
    let entries = std::fs::read_dir(&directory_path)
        .with_context(|| format!("Failed to read directory: {}", directory_path.display()))?;
    
    let mut files = Vec::new();
    
    for entry in entries {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();
        
        let name = path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid filename: {}", path.display()))?
            .to_string();
        
        let full_path = path.to_string_lossy().to_string();
        let is_file = path.is_file();
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_string());
        
        files.push(FileInfo {
            name,
            path: full_path,
            is_file,
            extension,
        });
    }
    
    // Sort files by name for consistent output
    files.sort_by(|a, b| a.name.cmp(&b.name));
    
    Ok(ListFilesInDirectoryResult {
        directory: directory_path.to_string_lossy().to_string(),
        files,
    })
}
