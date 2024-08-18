# Chat Log Parser

This Rust program parses chat log files from Google Voice or similar HTML-based chat logs. It extracts messages, participants, and other relevant information, providing various output formats for analysis.

## Features

- Parse individual chat log files or entire directories
- Support for one-on-one and group conversations
- Multiple output formats: Debug, JSON, and Default (with progress bar and statistics)
- Extracts message content, timestamps, participants, and labels
- Provides run statistics for batch processing

## Prerequisites

To run this program, you need to have Rust installed on your computer. If you don't have Rust installed, follow these steps:

1. Visit https://www.rust-lang.org/tools/install
2. Follow the instructions for your operating system to install Rust and Cargo (the Rust package manager)

## Installation

1. Clone this repository or download the source code:
   ```
   git clone https://github.com/yourusername/google_voice_importer.git
   cd google_voice_importer
   ```

2. Build the program using Cargo:
   ```
   cargo build --release
   ```

   This will compile the program and create an executable in the `target/release` directory.

## Usage

Run the program using the following command:
./target/release/google_voice_importer [OPTIONS] <INPUT>


Arguments:
- `<INPUT>`: Path to the input file or directory containing chat log files

Options:
- `--format <FORMAT>`: Output format [default: default] [possible values: debug, json, default]

### Examples

1. Parse a single file with default output:
   ```
   ./target/release/google_voice_importer path/to/chatlog.html
   ```

2. Parse a directory of chat logs with JSON output:
   ```
   ./target/release/google_voice_importer --format json path/to/chat/logs/directory
   ```

3. Parse a directory with debug output:
   ```
   ./target/release/google_voice_importer --format debug path/to/chat/logs/directory
   ```

## Output Formats

1. **Default**: Displays a progress bar while processing and prints statistics after completion.
2. **JSON**: Outputs the parsed data in JSON format for each file.
3. **Debug**: Prints a debug representation of the parsed data for each file.

