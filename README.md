# QBank - PDF Question Bank Processor

A Rust application that extracts questions from PDF files and processes them
through an AI endpoint to create a structured question bank stored in SQLite.

## Features

- **PDF Text Extraction**: Extracts text from PDF files page by page
- **AI Processing**: Sends extracted questions to LLM API for structured JSON output
- **SQLite Storage**: Stores questions with choices, explanations, and processing status
- **Per-Page Tracking**: Tracks processing status for each page, enabling retry of failed pages
- **Terminal UI**: Interactive TUI showing progress, page status, and logs
- **Configurable**: Settings for concurrent tasks, retries, API model, etc.

## Installation

### Prerequisites

- Rust 1.85+ (required for Rust 2024 edition)

### Build

```bash
cargo build --release
```

## Configuration

### Database Settings

The following settings are stored in SQLite and can be modified:

| Setting            | Default     | Description                                |
| ------------------ | ----------- | ------------------------------------------ |
| `max_retries`      | 3           | Maximum retry attempts for failed requests |
| `retry_delay_ms`   | 1000        | Base delay for exponential backoff (ms)    |
| `retry_multiplier` | 2.0         | Exponential backoff multiplier             |
| `api_model`        | deepseek-r1 | Model to use for API calls                 |

## Usage

### Basic Usage

```bash
# Process a PDF file
cargo run -- /path/to/your/file.pdf

# Retry failed pages
cargo run -- /path/to/your/file.pdf retry-failed
```

### Keyboard Controls

- `q` - Quit the application

## Output Format

Questions are stored with the following JSON structure:

```json
{
  "q": "Question text here",
  "c": [
    { "a": "Answer option 1", "r": true },
    { "a": "Answer option 2", "r": false },
    { "a": "Answer option 3", "r": false },
    { "a": "Answer option 4", "r": false }
  ],
  "explanation": "Detailed explanation of the correct answer"
}
```

## Project Structure

```
src/
├── main.rs           - Application entry point and TUI
├── cli.rs            - Command-line argument parsing
├── error.rs          - Custom error types
├── db/
│   ├── mod.rs       - Database initialization
│   ├── files.rs     - File operations
│   ├── pages.rs     - Page status tracking
│   ├── questions.rs - Question CRUD
│   └── settings.rs  - Configuration management
├── pdf/
│   ├── mod.rs       - PDF extraction wrapper
│   └── parser.rs    - Question parsing
├── processor/        - Core processing logic
│   ├── mod.rs       - Processing exports and helpers
│   ├── pdf.rs       - PDF page extraction
│   ├── page.rs      - Page processing logic
│   ├── question.rs  - Question validation
│   └── setup.rs     - TUI setup and runner
├── api/
│   └── client.rs    - LLM API client
└── tui/
    ├── mod.rs       - TUI exports
    ├── tui_loop.rs  - Main TUI loop
    ├── state.rs     - Application state
    └── widgets.rs   - TUI widgets
```

## License

MIT
