use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use elio_core::db_env::{DbConfig, DbEnv};
use elio_core::session::Session;
use futures::stream::StreamExt;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use tabled::settings::Style;
use tikv_jemallocator::Jemalloc;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[derive(Debug, Parser)]
#[command(author, version, about = "Elio - An embedded graph database", long_about = None)]
struct Args {
    #[arg(short, long, help = "Database path", default_value = ".elio")]
    db_path: String,
    #[arg(short = 'f', long = "file", help = "Read Cypher statements from file")]
    input_file: Option<PathBuf>,
}

fn print_help() {
    println!("Available commands:");
    println!("  .help       Show this help message");
    println!("  .quit       Exit the CLI");
    println!("  .exit       Exit the CLI");
    println!();
    println!("Enter Cypher queries to execute them.");
}

async fn execute_query(sess: &Arc<Session>, query: &str) {
    let start = Instant::now();

    match sess.execute(query.to_string(), HashMap::new()).await {
        Ok(mut result) => {
            let columns = result.columns().to_vec();
            let mut rows: Vec<Vec<String>> = Vec::new();

            while let Some(row_result) = result.next().await {
                match row_result {
                    Ok(row) => {
                        let formatted_row: Vec<String> = row
                            .iter()
                            .map(|v| match v {
                                Some(val) => val.to_string(),
                                None => "NULL".to_string(),
                            })
                            .collect();
                        rows.push(formatted_row);
                    }
                    Err(e) => {
                        eprintln!("Error reading row: {}", e);
                        return;
                    }
                }
            }

            let elapsed = start.elapsed();

            if columns.is_empty() {
                println!("Query executed successfully.");
            } else {
                print_table(&columns, &rows);
                println!("{} row(s)", rows.len());
            }

            println!("Executed in {:.3}s", elapsed.as_secs_f64());
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}

fn print_table(columns: &[String], rows: &[Vec<String>]) {
    if rows.is_empty() {
        // Print header only
        println!("{}", columns.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(" | "));
        return;
    }

    let mut builder = tabled::builder::Builder::new();

    // Add header
    builder.push_record(columns.iter().map(|s| s.as_str()));

    // Add rows
    for row in rows {
        builder.push_record(row.iter().map(|s| s.as_str()));
    }

    let table = builder.build().with(Style::rounded()).to_string();
    println!("{}", table);
}

async fn run_repl(sess: Arc<Session>) {
    let mut rl = match DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            eprintln!("Failed to initialize readline: {}", e);
            return;
        }
    };

    // Load history
    let history_path = dirs::home_dir()
        .map(|p| p.join(".elio_history"))
        .unwrap_or_else(|| ".elio_history".into());

    let _ = rl.load_history(&history_path);

    println!("Elio - An embedded graph database");
    println!("Type .help for available commands, .quit to exit");
    println!();

    loop {
        match rl.readline("elio> ") {
            Ok(line) => {
                let trimmed = line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(&line);

                match trimmed.to_lowercase().as_str() {
                    ".help" => print_help(),
                    ".quit" | ".exit" => {
                        println!("Goodbye!");
                        break;
                    }
                    _ => {
                        if trimmed.starts_with('.') {
                            eprintln!("Unknown command: {}", trimmed);
                            println!("Type .help for available commands");
                        } else {
                            execute_query(&sess, trimmed).await;
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("Goodbye!");
                break;
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }

    // Save history
    if let Err(e) = rl.save_history(&history_path) {
        eprintln!("Failed to save history: {}", e);
    }
}

fn statements_from_file(path: &PathBuf) -> Result<Vec<String>, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file {}: {}", path.display(), e))?;

    let mut statements = Vec::new();
    let mut current = String::new();

    for ch in content.chars() {
        current.push(ch);

        if ch == ';' {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                statements.push(trimmed.to_string());
            }
            current.clear();
        }
    }

    Ok(statements)
}

async fn run_file(sess: Arc<Session>, path: &PathBuf) {
    match statements_from_file(path) {
        Ok(statements) => {
            for statement in statements {
                println!("{}", statement);
                execute_query(&sess, statement.trim_end_matches(';')).await;
                println!();
            }
        }
        Err(e) => eprintln!("{}", e),
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let db_config = DbConfig::with_db_path(&args.db_path);

    let db = match DbEnv::open(&db_config) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open database: {}", e);
            std::process::exit(1);
        }
    };

    let sess = db.new_session();
    if let Some(path) = args.input_file {
        run_file(sess, &path).await;
    } else {
        run_repl(sess).await;
    }
}
