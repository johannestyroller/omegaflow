//! NEBRA-PILOT: The APPEAL Autonomous Compiler-Loop Agent.

use clap::Parser;
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;

// ---------------------------------------------------------------------------
// CLI Interface
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "nebra-pilot", about = "APPEAL Compiler-Loop Agent")]
struct Args {
    /// The file to fix
    #[arg(short, long)]
    file: PathBuf,

    /// Maximum fix iterations
    #[arg(short, long, default_value_t = 5)]
    max_iters: u8,
}

// ---------------------------------------------------------------------------
// Error Codes
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum PilotError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Cargo check failed after {0} iterations")]
    MaxIterationsExceeded(u8),
    #[error("API request failed: {0}")]
    ApiError(String),
}

// ---------------------------------------------------------------------------
// The Compiler Loop
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), PilotError> {
    let args = Args::parse();

    if !args.file.exists() {
        return Err(PilotError::FileNotFound(args.file.display().to_string()));
    }

    println!("🚀 NEBRA-PILOT: Initiating APPEAL Compiler-Loop for {}", args.file.display());

    for iteration in 1..=args.max_iters {
        println!("\n--- Iteration {}/{} ---", iteration, args.max_iters);

        // 1. Run cargo check
        let output = Command::new("cargo")
            .args(["check", "--message-format=short"])
            .output()
            .expect("Failed to execute cargo check");

        if output.status.success() {
            println!("✅ APPEAL COMPLIANCE: cargo check passed.");
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("❌ COMPILER ERROR:\n{}", stderr);

        // 2. TODO: Call LLM API with error + file content + APPEAL rules
        // 3. TODO: Apply LLM fix to file
        println!("⚙️ ANALYZING: API call not yet implemented. Waiting before retry...");
        
        // Prevent terminal spam while API is missing
        std::thread::sleep(std::time::Duration::from_secs(3));
    }

    Err(PilotError::MaxIterationsExceeded(args.max_iters))
}
