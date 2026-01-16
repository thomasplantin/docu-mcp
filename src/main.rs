pub mod config;
pub mod constants;
pub mod extractor;
pub mod extractors;
pub mod resources;
pub mod server;
pub mod tools;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // If run_server fails, it's a critical error (I/O failure, etc.) and we should exit with error code
    // This ensures the process fails loudly if the server can't start or run
    // All errors are logged to stderr so they're visible in Claude's UI
    if let Err(e) = server::run_server().await {
        eprintln!("[FATAL ERROR] Server crashed: {}", e);
        eprintln!("[FATAL ERROR] Error chain: {:#}", e);
        std::process::exit(1);
    }
    Ok(())
}
