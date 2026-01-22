# Contributing to Yali

Thank you for your interest in contributing to Yali! This document provides guidelines and instructions for contributing.

## Development Setup

### Prerequisites

- Rust 1.70 or later
- Cargo

### Getting Started

1. Fork the repository
2. Clone your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/yali.git
   cd yali
   ```

3. Build the project:
   ```bash
   cargo build
   ```

4. Run tests:
   ```bash
   cargo test
   ```

## Development Workflow

### Finding Work

1. Check [TODO.md](./TODO.md) for the current development roadmap
2. Look for issues labeled `good first issue` or `help wanted`
3. Pick an unchecked item from the current phase

### Making Changes

1. Create a feature branch:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. Make your changes with appropriate tests

3. Ensure all tests pass:
   ```bash
   cargo test
   ```

4. Check formatting:
   ```bash
   cargo fmt --check
   ```

5. Run clippy:
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```

### Submitting Changes

1. Commit your changes with clear, descriptive messages
2. Push to your fork
3. Create a Pull Request against the `main` branch
4. Ensure CI passes

## Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Address all clippy warnings
- Write tests for new functionality
- Document public APIs with doc comments

## Testing

### Running Tests

```bash
# All tests
cargo test

# Unit tests only
cargo test --lib

# Integration tests only
cargo test --test integration_test

# With verbose output
cargo test -- --nocapture
```

### Writing Tests

- Unit tests go in the same file as the code being tested
- Integration tests go in the `tests/` directory
- Use descriptive test names that explain what's being tested

## Project Structure

```
yali/
├── src/
│   ├── main.rs          # Binary entry point
│   ├── lib.rs           # Library exports
│   ├── config/          # Configuration parsing
│   ├── state.rs         # Gateway state management
│   └── proxy.rs         # Pingora proxy implementation
├── tests/               # Integration tests
├── .github/workflows/   # CI configuration
├── TODO.md              # Development roadmap
└── prd                  # Product Requirements Document
```

## Questions?

Open an issue if you have questions or need clarification on anything.
