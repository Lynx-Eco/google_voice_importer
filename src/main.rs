use std::fs::File;
use std::io::{ self, BufRead };
use std::path::{ Path, PathBuf };
use chrono::NaiveDateTime;
use clap::{ Parser, ValueEnum };
use walkdir::WalkDir;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct Message {
    from: String,
    to: String,
    timestamp: NaiveDateTime,
    content: String,
}

#[derive(Debug, Serialize)]
struct Thread {
    messages: Vec<Message>,
    labels: Vec<String>,
    message_count: usize,
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Input file or directory
    input: PathBuf,

    /// Output format
    #[clap(value_enum, default_value_t = OutputFormat::Debug)]
    format: OutputFormat,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize)]
enum OutputFormat {
    Debug,
    Json,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    if cli.input.is_dir() {
        for entry in WalkDir::new(&cli.input)
            .into_iter()
            .filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                process_file(entry.path(), &cli.format)?;
            }
        }
    } else {
        process_file(&cli.input, &cli.format)?;
    }

    Ok(())
}

fn process_file(file_path: &Path, format: &OutputFormat) -> io::Result<()> {
    println!("Processing file: {:?}", file_path);
    let thread = parse_file(file_path)?;

    match format {
        OutputFormat::Debug => println!("{:#?}", thread),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&thread).unwrap()),
    }

    Ok(())
}

fn parse_file<P: AsRef<Path>>(filename: P) -> io::Result<Thread> {
    let file = File::open(filename)?;
    let reader = io::BufReader::new(file);
    let mut messages = Vec::new();
    let mut labels = Vec::new();
    let mut current_message: Option<Message> = None;

    for line in reader.lines() {
        let line = line?;
        if line.starts_with("Labels:") {
            labels = line
                .split(':')
                .nth(1)
                .unwrap_or("")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            continue;
        }
        if line.starts_with("User Deleted:") {
            // Skip this line
            continue;
        }

        if let Some(date_time) = line.split(": ").next() {
            if let Ok(timestamp) = parse_flexible_datetime(date_time) {
                if let Some(message) = current_message.take() {
                    messages.push(message);
                }

                let parts: Vec<&str> = line.splitn(3, ": ").collect();
                if parts.len() == 3 {
                    current_message = Some(Message {
                        from: parts[1].to_string(),
                        to: "Unknown".to_string(),
                        timestamp,
                        content: parts[2].to_string(),
                    });
                }
            } else if let Some(ref mut message) = current_message {
                message.content.push('\n');
                message.content.push_str(&line);
            }
        } else if let Some(ref mut message) = current_message {
            message.content.push('\n');
            message.content.push_str(&line);
        }
    }

    if let Some(message) = current_message {
        messages.push(message);
    }
    Ok(Thread {
        messages: messages.clone(),
        labels,
        message_count: messages.len(),
    })
}
fn parse_flexible_datetime(date_time: &str) -> Result<NaiveDateTime, anyhow::Error> {
    let formats = [
        "%b %d, %Y, %I:%M:%S %p Pacific Time",
        "%b %d, %Y, %I:%M:%S %p %Z",
        "%b %d, %Y, %I:%M:%S %p",
        "%Y-%m-%d %H:%M:%S %Z",
        "%Y-%m-%d %H:%M:%S",
    ];

    for format in &formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(date_time, format) {
            return Ok(dt);
        }
    }
    Err(anyhow::anyhow!("Failed to parse datetime: {}", date_time))
}
