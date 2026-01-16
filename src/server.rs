use std::io::{self, BufRead, Write};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::tools::{
    set_document_directory, list_document_directories, extract_text_from_file, list_files_in_directory,
    SetDocumentDirectoryParams, ExtractTextFromFileParams, ListFilesInDirectoryParams,
};
use crate::resources::{list_resources, get_resource};

/// JSON-RPC request structure
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

/// JSON-RPC response structure
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC error structure
#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// MCP Initialize request parameters
#[derive(Debug, Deserialize)]
struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,
    capabilities: Option<Value>,
    #[serde(rename = "clientInfo")]
    client_info: Option<Value>,
}

/// MCP Initialize result
#[derive(Debug, Serialize)]
struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,
    capabilities: InitializeCapabilities,
    #[serde(rename = "serverInfo")]
    server_info: ServerInfo,
}

/// MCP Initialize capabilities
#[derive(Debug, Serialize)]
struct InitializeCapabilities {
    tools: Option<Value>,
    resources: Option<Value>,
}

/// MCP Server info
#[derive(Debug, Serialize)]
struct ServerInfo {
    name: String,
    version: String,
}

/// MCP Tool definition
#[derive(Debug, Serialize)]
struct Tool {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}


/// Run the MCP server with JSON-RPC stdio communication
pub async fn run_server() -> Result<()> {
    let stdin = io::stdin();
    let mut stdin_lock = stdin.lock();
    let mut stdout = io::stdout();
    
    let mut initialized = false;
    
    loop {
        let mut line = String::new();
        let bytes_read = stdin_lock.read_line(&mut line)?;
        
        if bytes_read == 0 {
            // EOF
            break;
        }
        
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        
        // Parse JSON-RPC request
        let request: JsonRpcRequest = match serde_json::from_str::<JsonRpcRequest>(line) {
            Ok(req) => {
                // Validate JSON-RPC version
                if req.jsonrpc != "2.0" {
                    eprintln!("[ERROR] Invalid JSON-RPC version: {}. Expected 2.0", req.jsonrpc);
                    let error_response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req.id.clone(),
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32600,
                            message: format!("Invalid JSON-RPC version: {}. Expected 2.0", req.jsonrpc),
                            data: None,
                        }),
                    };
                    let response_json = serde_json::to_string(&error_response)
                        .context("Failed to serialize error response - critical error")?;
                    writeln!(stdout, "{}", response_json)
                        .context("Failed to write error response to stdout - critical I/O error")?;
                    stdout.flush()
                        .context("Failed to flush stdout - critical I/O error")?;
                    continue;
                }
                req
            }
            Err(e) => {
                // Log parse error to stderr so it's visible in Claude's UI
                eprintln!("[ERROR] Failed to parse JSON-RPC request: {}", e);
                eprintln!("[ERROR] Invalid JSON line: {}", line);
                
                // Send error response for invalid JSON
                let error_response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: "Parse error".to_string(),
                        data: Some(Value::String(e.to_string())),
                    }),
                };
                let response_json = serde_json::to_string(&error_response)
                    .context("Failed to serialize error response - critical error")?;
                writeln!(stdout, "{}", response_json)
                    .context("Failed to write error response to stdout - critical I/O error")?;
                stdout.flush()
                    .context("Failed to flush stdout - critical I/O error")?;
                continue;
            }
        };
        
        // Handle notifications (requests without IDs) - no response needed
        if request.id.is_none() {
            if let Err(e) = handle_notification(&request, &mut initialized) {
                // Log notification errors to stderr so they're visible in Claude's UI
                eprintln!("[ERROR] Notification '{}' failed: {}", request.method, e);
            }
            continue;
        }
        
        // Handle requests (with IDs) - must send a response
        // Note: Errors in handle_request are expected (bad requests, missing files, etc.)
        // and should return error responses, not crash the server.
        // Critical I/O errors (stdin/stdout) will still propagate and crash, which is correct.
        let response = match handle_request(&request, &mut initialized) {
            Ok(resp) => resp,
            Err(e) => {
                // Log error to stderr so it's visible in Claude's UI
                eprintln!("[ERROR] Request '{}' failed: {}", request.method, e);
                
                // Send error response for request handling errors
                // These are expected errors (invalid params, missing files, etc.)
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32000,
                        message: format!("Request failed: {}", e),
                        data: Some(Value::String(e.to_string())),
                    }),
                }
            }
        };
        
        // Send response - if this fails, it's a critical I/O error and should crash
        let response_json = serde_json::to_string(&response)
            .context("Failed to serialize response - critical error")?;
        writeln!(stdout, "{}", response_json)
            .context("Failed to write response to stdout - critical I/O error")?;
        stdout.flush()
            .context("Failed to flush stdout - critical I/O error")?;
    }
    
    Ok(())
}

/// Handle a JSON-RPC notification (no response needed)
fn handle_notification(request: &JsonRpcRequest, initialized: &mut bool) -> Result<()> {
    match request.method.as_str() {
        "initialized" | "notifications/initialized" => {
            // Client has finished initialization - server can now send requests if needed
            // Handle both "initialized" and "notifications/initialized" for compatibility
            if !*initialized {
                return Err(anyhow::anyhow!("Received initialized notification before initialize request"));
            }
            Ok(())
        }
        _ => {
            // Unknown notification - ignore it (per JSON-RPC spec)
            Ok(())
        }
    }
}

/// Handle a JSON-RPC request
fn handle_request(request: &JsonRpcRequest, initialized: &mut bool) -> Result<JsonRpcResponse> {
    match request.method.as_str() {
        "initialize" => {
            if *initialized {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32000,
                        message: "Already initialized".to_string(),
                        data: None,
                    }),
                });
            }
            
            let params: InitializeParams = serde_json::from_value(
                request.params.clone().unwrap_or(Value::Object(serde_json::Map::new()))
            ).context("Failed to parse initialize params")?;
            
            // Validate protocol version (accept common MCP protocol versions)
            // Accept versions: 2024-11-05, 2025-06-18, 2025-11-25
            let supported_versions = ["2024-11-05", "2025-06-18", "2025-11-25"];
            if !supported_versions.contains(&params.protocol_version.as_str()) {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32000,
                        message: format!("Unsupported protocol version: {}. Supported versions: {}", params.protocol_version, supported_versions.join(", ")),
                        data: None,
                    }),
                });
            }
            
            // Acknowledge client capabilities and info (for future extensibility)
            // Currently we support all standard MCP capabilities
            if let Some(ref caps) = params.capabilities {
                // Client capabilities received - can be used for negotiation in future
                let _ = caps;
            }
            if let Some(ref info) = params.client_info {
                // Client info received - can be used for logging/debugging in future
                let _ = info;
            }
            
            *initialized = true;
            
            let result = InitializeResult {
                protocol_version: params.protocol_version.clone(),
                capabilities: InitializeCapabilities {
                    tools: Some(serde_json::json!({
                        "listChanged": true
                    })),
                    resources: Some(serde_json::json!({
                        "subscribe": true,
                        "listChanged": true
                    })),
                },
                server_info: ServerInfo {
                    name: "docu-mcp".to_string(),
                    version: "0.1.0".to_string(),
                },
            };
            
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: Some(serde_json::to_value(result)?),
                error: None,
            })
        }
        
        "tools/list" => {
            if !*initialized {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32002,
                        message: "Not initialized".to_string(),
                        data: None,
                    }),
                });
            }
            
            let tools = vec![
                Tool {
                    name: "set_document_directory".to_string(),
                    description: "Set the active document directory. Validates directory exists and is readable, adds to directories list if not present, sets as active_directory, and saves config.".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "directory": {
                                "type": "string",
                                "description": "Path to directory"
                            }
                        },
                        "required": ["directory"]
                    }),
                },
                Tool {
                    name: "list_document_directories".to_string(),
                    description: "List all document directories and the active directory.".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
                Tool {
                    name: "extract_text_from_file".to_string(),
                    description: "Extract text from a document file using the appropriate extractor.".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "file_path": {
                                "type": "string",
                                "description": "Path to the file to extract text from"
                            }
                        },
                        "required": ["file_path"]
                    }),
                },
                Tool {
                    name: "list_files_in_directory".to_string(),
                    description: "List all files and subdirectories in a directory. If no directory is provided, uses the active directory.".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "directory": {
                                "type": "string",
                                "description": "Optional directory path. If not provided, uses the active directory."
                            }
                        },
                        "required": []
                    }),
                },
            ];
            
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: Some(serde_json::json!({ "tools": tools })),
                error: None,
            })
        }
        
        "tools/call" => {
            if !*initialized {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32002,
                        message: "Not initialized".to_string(),
                        data: None,
                    }),
                });
            }
            
            let params = request.params.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing params for tools/call"))?;
            
            let tool_name = params.get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing tool name"))?;
            
            let arguments = params.get("arguments")
                .cloned()
                .unwrap_or(Value::Object(serde_json::Map::new()));
            
            let result = match tool_name {
                "set_document_directory" => {
                    let params: SetDocumentDirectoryParams = serde_json::from_value(arguments)
                        .context("Failed to parse set_document_directory params")?;
                    let result = set_document_directory(params)?;
                    serde_json::to_value(result)?
                }
                "list_document_directories" => {
                    let result = list_document_directories()?;
                    serde_json::to_value(result)?
                }
                "extract_text_from_file" => {
                    let params: ExtractTextFromFileParams = serde_json::from_value(arguments)
                        .context("Failed to parse extract_text_from_file params")?;
                    let result = extract_text_from_file(params)?;
                    serde_json::to_value(result)?
                }
                "list_files_in_directory" => {
                    let params: ListFilesInDirectoryParams = serde_json::from_value(arguments)
                        .context("Failed to parse list_files_in_directory params")?;
                    let result = list_files_in_directory(params)?;
                    serde_json::to_value(result)?
                }
                _ => {
                    return Ok(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id.clone(),
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: format!("Unknown tool: {}", tool_name),
                            data: None,
                        }),
                    });
                }
            };
            
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: Some(serde_json::json!({ "content": [{ "type": "text", "text": serde_json::to_string(&result)? }] })),
                error: None,
            })
        }
        
        "resources/list" => {
            if !*initialized {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32002,
                        message: "Not initialized".to_string(),
                        data: None,
                    }),
                });
            }
            
            match list_resources() {
                Ok(resources) => {
                    Ok(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id.clone(),
                        result: Some(serde_json::json!({ "resources": resources })),
                        error: None,
                    })
                }
                Err(e) => {
                    // If no active directory is set, return empty list instead of error
                    // This allows Claude to enable the connector even without a directory set
                    let error_msg = e.to_string();
                    if error_msg.contains("No active directory set") {
                        Ok(JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id.clone(),
                            result: Some(serde_json::json!({ "resources": [] })),
                            error: None,
                        })
                    } else {
                        // Log other errors to stderr
                        eprintln!("[ERROR] Failed to list resources: {}", e);
                        Ok(JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id.clone(),
                            result: None,
                            error: Some(JsonRpcError {
                                code: -32000,
                                message: format!("Failed to list resources: {}", e),
                                data: Some(Value::String(e.to_string())),
                            }),
                        })
                    }
                }
            }
        }
        
        "resources/read" => {
            if !*initialized {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32002,
                        message: "Not initialized".to_string(),
                        data: None,
                    }),
                });
            }
            
            let params = request.params.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing params for resources/read"))?;
            
            let uri = params.get("uri")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing URI"))?;
            
            match get_resource(uri) {
                Ok(resource_content) => {
                    Ok(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id.clone(),
                        result: Some(serde_json::json!({
                            "contents": [{
                                "uri": resource_content.uri,
                                "mimeType": resource_content.mime_type,
                                "text": resource_content.text
                            }]
                        })),
                        error: None,
                    })
                }
                Err(e) => {
                    Ok(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id.clone(),
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32000,
                            message: format!("Failed to read resource: {}", e),
                            data: Some(Value::String(e.to_string())),
                        }),
                    })
                }
            }
        }
        
        _ => {
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Unknown method: {}", request.method),
                    data: None,
                }),
            })
        }
    }
}
