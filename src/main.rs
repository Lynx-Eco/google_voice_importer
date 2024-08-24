use anyhow::{ Context, Result };
use chrono::{ DateTime, Utc };
use clap::Parser;
use indicatif::{ ProgressBar, ProgressStyle };
use scraper::{ Html, Selector };
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{ Path, PathBuf };
use std::time::Instant;
use walkdir::WalkDir;
use glob::glob;

// Constants for HTML selectors and date format

const MESSAGE_SELECTOR: &str = ".message";
const DATETIME_SELECTOR: &str = ".dt";
const SENDER_SELECTOR: &str = ".sender";
const CONTENT_SELECTOR: &str = "q";
const TAGS_SELECTOR: &str = ".tags";
const TEXT_LABEL: &str = "- Text -";
const GROUP_CONVERSATION_LABEL: &str = "Group Conversation -";
const DATETIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.3f%:z";

// Struct to represent a participant in the conversation
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
struct Participant {
    name: String,
    phone: String,
}

// Struct to represent a single message
#[derive(Debug, Clone, Serialize)]
struct Message {
    from: Participant,
    to: Vec<Participant>,
    timestamp: DateTime<Utc>,
    content: String,
}

// Struct to represent a thread of messages
#[derive(Debug, Serialize)]
struct Thread {
    messages: Vec<Message>,
    participants: Vec<Participant>,
    labels: Vec<String>,
    message_count: usize,
}

// Command-line interface struct
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Input file or directory
    input: PathBuf,

    /// Output format
    #[clap(value_enum, default_value_t = OutputFormat::Default)]
    format: OutputFormat,
}

// Enum to represent different output formats
#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
enum OutputFormat {
    Debug,
    Json,
    Default,
}

// Struct to hold statistics about the processing run
#[derive(Debug)]
struct RunStatistics {
    duration: std::time::Duration,
    files_processed: usize,
    messages_extracted: usize,
    unique_participants: usize,
    avg_messages_per_file: f64,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Expand the input path, including any glob patterns
    let expanded_paths: Vec<PathBuf> = glob(&cli.input.to_string_lossy())
        .with_context(|| format!("Failed to read glob pattern: {:?}", cli.input))?
        .filter_map(Result::ok)
        .collect();

    if expanded_paths.is_empty() {
        anyhow::bail!("No matching paths found for: {:?}", cli.input);
    }

    for path in expanded_paths {
        if path.is_dir() {
            process_directory(&path, &cli.format)?;
        } else if path.is_file() {
            process_file(&path, &cli.format)?;
        } else {
            println!("Skipping non-file, non-directory: {:?}", path);
        }
    }

    Ok(())
}

fn process_directory(dir: &Path, format: &OutputFormat) -> Result<()> {
    let start_time = Instant::now();
    let mut files_processed = 0;
    let mut total_messages = 0;
    let mut all_participants = HashSet::new();

    // Get the list of files to process
    let files: Vec<_> = WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file() &&
                (e.file_name().to_str().unwrap_or("").contains(TEXT_LABEL) ||
                    e.file_name().to_str().unwrap_or("").contains(GROUP_CONVERSATION_LABEL))
        })
        .collect();

    // Set up progress bar for Default format
    let progress_bar = if *format == OutputFormat::Default {
        let pb = ProgressBar::new(files.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})"
                )
                .unwrap()
                .progress_chars("#>-")
        );
        Some(pb)
    } else {
        None
    };

    // Process each file
    for entry in files {
        let thread = parse_file(entry.path()).context("Failed to parse file")?;
        match format {
            OutputFormat::Debug => println!("{:#?}", thread),
            OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&thread)?),
            OutputFormat::Default => {
                if let Some(pb) = &progress_bar {
                    pb.inc(1);
                }
            }
        }

        files_processed += 1;
        total_messages += thread.messages.len();
        all_participants.extend(thread.participants.iter().cloned());
    }

    // Finalize progress bar
    if let Some(pb) = progress_bar {
        pb.finish_with_message("Processing complete");
    }

    // Calculate and display statistics for Default format
    if *format == OutputFormat::Default {
        let duration = start_time.elapsed();
        let stats = RunStatistics {
            duration,
            files_processed,
            messages_extracted: total_messages,
            unique_participants: all_participants.len(),
            avg_messages_per_file: (total_messages as f64) / (files_processed as f64),
        };
        print_statistics(&stats);
    }

    Ok(())
}

fn process_file(file_path: &Path, format: &OutputFormat) -> Result<()> {
    println!("Processing file: {:?}", file_path);
    let thread = parse_file(file_path).context("Failed to parse file")?;

    match format {
        OutputFormat::Debug => println!("{:#?}", thread),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&thread)?),
        OutputFormat::Default => {
            println!("Processed 1 file with {} messages", thread.messages.len());
        }
    }

    Ok(())
}

fn parse_file(filename: &Path) -> Result<Thread> {
    let content = fs::read_to_string(filename).context("Failed to read file")?;
    let document = Html::parse_document(&content);

    // Initialize selectors
    let message_selector = Selector::parse(MESSAGE_SELECTOR).unwrap();
    let dt_selector = Selector::parse(DATETIME_SELECTOR).unwrap();
    let sender_selector = Selector::parse(SENDER_SELECTOR).unwrap();
    let q_selector = Selector::parse(CONTENT_SELECTOR).unwrap();

    let mut participants = HashSet::new();
    let mut me_participant = None;

    // First pass: Collect all participants
    for message_element in document.select(&message_selector) {
        let sender_element = message_element.select(&sender_selector).next().unwrap();
        let phone_number = sender_element
            .select(&Selector::parse("a.tel").unwrap())
            .next()
            .and_then(|el| el.value().attr("href"))
            .and_then(|href| href.strip_prefix("tel:"))
            .unwrap_or("Unknown");

        let name = sender_element
            .select(&Selector::parse("span.fn, abbr.fn").unwrap())
            .next()
            .and_then(|el| el.text().next())
            .unwrap_or("");

        let participant = Participant {
            name: name.to_string(),
            phone: phone_number.to_string(),
        };

        if name == "Me" {
            me_participant = Some(participant.clone());
        }

        participants.insert(participant);
    }

    let participants: Vec<Participant> = participants.into_iter().collect();
    let me_participant = me_participant.unwrap_or_else(|| {
        Participant {
            name: "Me".to_string(),
            phone: "Unknown".to_string(),
        }
    });

    // Second pass: Parse messages
    let messages = document
        .select(&message_selector)
        .map(|message_element| {
            let timestamp = message_element
                .select(&dt_selector)
                .next()
                .and_then(|el| el.value().attr("title"))
                .and_then(|date_str| DateTime::parse_from_str(date_str, DATETIME_FORMAT).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|| DateTime::from_timestamp(0, 0).expect("Invalid timestamp"));

            let sender_element = message_element.select(&sender_selector).next().unwrap();
            let phone_number = sender_element
                .select(&Selector::parse("a.tel").unwrap())
                .next()
                .and_then(|el| el.value().attr("href"))
                .and_then(|href| href.strip_prefix("tel:"))
                .unwrap_or("Unknown");

            let from = participants
                .iter()
                .find(|p| p.phone == phone_number)
                .unwrap_or(&me_participant)
                .clone();

            let to = if from == me_participant {
                participants
                    .iter()
                    .filter(|&p| p != &me_participant)
                    .cloned()
                    .collect()
            } else {
                vec![me_participant.clone()]
            };

            let content = message_element
                .select(&q_selector)
                .next()
                .map(|el| el.text().collect::<String>())
                .unwrap_or_default();

            Message {
                from,
                to,
                timestamp,
                content,
            }
        })
        .collect::<Vec<_>>();

    let labels = parse_labels(&document);

    Ok(Thread {
        messages: messages.clone(),
        participants,
        labels,
        message_count: messages.len(),
    })
}

fn parse_labels(document: &Html) -> Vec<String> {
    let tags_selector = Selector::parse(TAGS_SELECTOR).unwrap();
    document
        .select(&tags_selector)
        .next()
        .map(|tags_element| {
            tags_element
                .text()
                .collect::<String>()
                .split(':')
                .nth(1)
                .unwrap_or("")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn print_statistics(stats: &RunStatistics) {
    println!("\nRun Statistics:");
    println!("Duration: {:?}", stats.duration);
    println!("Files processed: {}", stats.files_processed);
    println!("Messages extracted: {}", stats.messages_extracted);
    println!("Unique participants: {}", stats.unique_participants);
    println!("Average messages per file: {:.2}", stats.avg_messages_per_file);
}
