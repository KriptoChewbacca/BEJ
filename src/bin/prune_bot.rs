//! Prune Bot - Quantum-Inspired Static Path Pruner
//!
//! CLI tool for analyzing and pruning low-probability async paths in Rust code.

use anyhow::Result;
use bot::components::quantum_pruner::PathPruner;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about = "Quantum-Inspired Static Path Pruner", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Analyze a directory for prunable paths
    Analyze {
        /// Directory to analyze
        #[arg(default_value = "src")]
        path: PathBuf,

        /// Probability threshold for pruning (0.0 to 1.0)
        #[arg(short, long, default_value = "0.01")]
        threshold: f64,
    },

    /// Generate pruning report
    Report {
        /// Directory to analyze
        #[arg(default_value = "src")]
        path: PathBuf,

        /// Output file for report
        #[arg(short, long, default_value = "prune_report.md")]
        output: PathBuf,
    },

    /// Get pruning suggestions
    Suggest {
        /// Directory to analyze
        #[arg(default_value = "src")]
        path: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let level = if args.verbose { Level::DEBUG } else { Level::INFO };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    match args.command {
        Commands::Analyze { path, threshold } => {
            info!(
                "Analyzing directory: {:?} with threshold {:.2}%",
                path,
                threshold * 100.0
            );

            let pruner = PathPruner::new(threshold);
            let analysis = pruner.analyze_directory(&path)?;

            println!("\n=== Path Pruning Analysis ===\n");
            println!("Directory: {}", analysis.directory.display());
            println!("Files analyzed: {}", analysis.files_analyzed);
            println!("Total paths: {}", analysis.total_paths);
            println!(
                "Low-probability paths: {}",
                analysis.total_low_probability_paths
            );
            println!(
                "Pruning potential: {:.1}%\n",
                (analysis.total_low_probability_paths as f64
                    / analysis.total_paths.max(1) as f64)
                    * 100.0
            );

            // Show top files with prunable paths
            println!("Top files with prunable paths:");
            let mut sorted_files: Vec<_> = analysis
                .file_analyses
                .iter()
                .filter(|f| f.low_probability_paths > 0)
                .collect();
            sorted_files.sort_by_key(|f| std::cmp::Reverse(f.low_probability_paths));

            for (i, file) in sorted_files.iter().take(10).enumerate() {
                println!(
                    "{}. {} - {} paths ({:.1}%)",
                    i + 1,
                    file.file_path.display(),
                    file.low_probability_paths,
                    (file.low_probability_paths as f64 / file.total_paths.max(1) as f64) * 100.0
                );
            }
        }

        Commands::Report { path, output } => {
            info!("Generating report for: {:?}", path);

            let pruner = PathPruner::default();
            let analysis = pruner.analyze_directory(&path)?;
            let report = pruner.generate_report(&analysis);

            std::fs::write(&output, report)?;
            println!("Report generated: {}", output.display());
        }

        Commands::Suggest { path, format } => {
            info!("Generating suggestions for: {:?}", path);

            let pruner = PathPruner::default();
            let analysis = pruner.analyze_directory(&path)?;
            let suggestions = pruner.get_suggestions(&analysis);

            match format.as_str() {
                "json" => {
                    let json = serde_json::to_string_pretty(&suggestions)?;
                    println!("{}", json);
                }
                _ => {
                    println!("\n=== Pruning Suggestions ===\n");
                    for (i, suggestion) in suggestions.iter().enumerate() {
                        println!(
                            "{}. {}:{}",
                            i + 1,
                            suggestion.file.display(),
                            suggestion.line
                        );
                        println!("   Type: {:?}", suggestion.suggestion_type);
                        println!("   Impact: {:?}", suggestion.impact);
                        println!("   Original: {}", suggestion.original_code);
                        println!("   Suggested: {}", suggestion.suggested_code);
                        println!();
                    }
                }
            }
        }
    }

    Ok(())
}
