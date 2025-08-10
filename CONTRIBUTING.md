# Contributing to Terminator

Welcome to Terminator! We appreciate your interest in contributing to this AI-native GUI automation framework. This guide will help you get started with contributing to the project.

## 🚀 Release Management

We use a **cross-platform Rust CLI** for version management. The cleanest commands:

### Quick Release (Most Common)

```bash
terminator release                     # Bump patch version + tag + push for CI
```

### Manual Workflow

```bash
terminator status                      # Check current versions
terminator patch                       # Bump patch version (x.y.Z+1)
terminator sync                        # Sync all package versions
terminator tag                         # Tag and push current version
```

### Installation (Run Once)

```bash
cargo install --path terminator-cli   # Install globally to PATH
```

### CLI Help

```bash
terminator --help                      # Show all commands and options
```

### Alternative Commands (More Verbose)

```bash
cargo terminator status               # Same as: terminator status
cargo run --bin terminator -- status  # Same as: terminator status
```

### What It Does Automatically

1. **Version bumping** - Semantic versioning (patch/minor/major)
2. **Syncs all packages** - Workspace → Node.js bindings → MCP agent → Platform packages
3. **Git operations** - Auto-commit, tag creation, push to trigger CI
4. **Status checking** - See all versions at a glance

## 🚀 Getting Started

### Prerequisites

- **Rust** (latest stable version) - [Install Rust](https://rustup.rs/)
- **Node.js** (for TypeScript bindings) - [Install Node.js](https://nodejs.org/)
- **Python 3.8+** (for Python bindings) - [Install Python](https://python.org/)

### Optional: Speed Up Builds with sccache

For faster compilation, you can optionally install [sccache](https://github.com/mozilla/sccache), a distributed compilation cache:

#### Installation

**Via Cargo:**

```bash
cargo install sccache
```

**Via Package Managers:**

```bash
# Windows (Chocolatey)
choco install sccache

# Windows (Scoop)
scoop install sccache

# macOS (Homebrew)
brew install sccache

# Linux (most distributions)
# Download latest release from: https://github.com/mozilla/sccache/releases
```

#### Setup

**Temporary (current session only):**

```bash
export RUSTC_WRAPPER=sccache
```

**Permanent setup:**

Add to your shell profile (`~/.bashrc`, `~/.zshrc`, or PowerShell profile):

```bash
# Bash/Zsh
export RUSTC_WRAPPER=sccache

# PowerShell
$env:RUSTC_WRAPPER = "sccache"
```

#### Verify Installation

```bash
sccache --version
sccache --show-stats  # Shows cache statistics
```

**Note:** sccache is completely optional. The project builds fine without it, but it can significantly speed up compilation times, especially for incremental builds and when working across multiple projects.

- **Git** - [Install Git](https://git-scm.com/)
- **Windows 10/11** (for full testing, though development can happen on other platforms)

### Development Setup

1. **Clone the repository:**

   ```bash
   git clone https://github.com/mediar-ai/terminator.git
   cd terminator
   ```

2. **Build the workspace:**

   ```bash
   cargo build
   ```

3. **Run tests:**

   ```bash
   cargo test
   ```

4. **Set up language bindings:**

   ```bash
   # Python bindings
   cd bindings/python
   pip install -e .

   # Node.js bindings
   cd ../nodejs
   npm install
   npm run build
   ```

## 🏗️ Project Structure

Terminator uses a Cargo workspace with the following key components:

- `terminator/` - Core Rust library
- `bindings/python/` - Python wrapper
- `bindings/nodejs/` - TypeScript/Node.js wrapper
- `examples/` - Usage examples and integration tests
- `terminator-workflow-recorder/` - Workflow recording tool

## 🔧 Development Guidelines

### Code Style

- **Rust**: Follow standard Rust formatting (`cargo fmt`)
- **Python**: Follow PEP 8 and use `black` for formatting
- **TypeScript**: Use Prettier and ESLint configurations

### Commit Messages

Use conventional commits format:

```
type(scope): description

feat(core): add new locator strategy for accessibility
fix(python): resolve memory leak in element caching
docs(readme): update installation instructions
```

### API Design Principles

1. **Async by Default**: All automation operations should be async
2. **Fluent Interface**: Design chainable APIs where appropriate
3. **Clear Error Messages**: Provide actionable error information
4. **Platform Abstraction**: Use traits to abstract platform differences

## 🧪 Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p terminator

# Run integration tests
cargo test --test integration_tests
```

### Test Guidelines

- **Unit Tests**: Test individual functions and modules
- **Integration Tests**: Test against real applications (Calculator, Notepad)
- **Examples as Tests**: Ensure all examples in `examples/` work correctly
- **Platform Testing**: Test on Windows for full functionality

### Adding New Tests

1. Add unit tests alongside your code in `src/` files
2. Add integration tests in `tests/` directories
3. Create examples in `examples/` for new features
4. Ensure tests pass on CI before submitting PRs

## 📝 Documentation

### Code Documentation

- Document all public APIs with doc comments
- Include usage examples in doc comments
- Explain platform-specific behavior
- Update README.md for user-facing changes

### Documentation Format

````rust
/// Locates an element on the desktop using the specified selector.
///
/// # Arguments
///
/// * `selector` - A selector string (e.g., "name:Button", "id:submit")
///
/// # Examples
///
/// ```rust
/// let button = desktop.locator("name:Save").await?;
/// button.click().await?;
/// ```
///
/// # Platform Notes
///
/// On Windows, uses UIAutomation. On macOS, uses Accessibility APIs.
pub async fn locator(&self, selector: &str) -> Result<Element> {
    // implementation
}
````

## 🐛 Reporting Issues

### Bug Reports

When reporting bugs, please include:

1. **Environment**: OS version, Rust version, target application
2. **Steps to Reproduce**: Minimal example that demonstrates the issue
3. **Expected Behavior**: What should happen
4. **Actual Behavior**: What actually happens
5. **Error Messages**: Full error messages and stack traces

### Feature Requests

For feature requests, please describe:

1. **Use Case**: Why is this feature needed?
2. **Proposed Solution**: How should it work?
3. **Alternatives**: What alternatives have you considered?

## 🔄 Pull Request Process

### Before Submitting

1. **Fork** the repository
2. **Create a branch** from `main` with a descriptive name
3. **Make your changes** following the guidelines above
4. **Test thoroughly** on your target platform
5. **Update documentation** as needed

### Submission Checklist

- [ ] Code follows project style guidelines
- [ ] Tests pass locally (`cargo test`)
- [ ] New tests added for new functionality
- [ ] Documentation updated (including README if applicable)
- [ ] Commit messages follow conventional format
- [ ] PR description explains the changes clearly

### Review Process

1. **Automated Checks**: CI will run tests and linting
2. **Code Review**: Maintainers will review your code
3. **Discussion**: Address any feedback or questions
4. **Merge**: Once approved, your PR will be merged

## 🤝 Community

- **Discord**: Join our [Discord server](https://discord.gg/dU9EBuw7Uq) for discussions
- **Issues**: Use GitHub Issues for bug reports and feature requests
- **Discussions**: Use GitHub Discussions for questions and ideas

## 📄 License

By contributing to Terminator, you agree that your contributions will be licensed under the same license as the project.

## 🙏 Recognition

All contributors will be recognized in our documentation and release notes. Thank you for making Terminator better!

---

_Need help? Join our [Discord](https://discord.gg/dU9EBuw7Uq) or open an issue!_

## 🚀 Release Management

We use a **cross-platform Rust CLI** for version management. All commands run from the workspace root.

### Quick Release (Most Common)

```bash
cargo terminator release                  # Bump patch version + tag + push for CI
```

### Manual Workflow

```bash
cargo terminator status                   # Check current versions
cargo terminator patch                    # Bump patch version (x.y.Z+1)
cargo terminator sync                     # Sync all package versions
cargo terminator tag                      # Tag and push current version
```

### Alternative (Verbose) Commands

```bash
cargo run --bin terminator -- release    # Same as cargo terminator release
cargo run --bin terminator -- status     # Same as cargo terminator status
# etc...
```

### Available Commands

- `patch` - Bump patch version (x.y.Z+1)
- `minor` - Bump minor version (x.Y+1.0)
- `major` - Bump major version (X+1.0.0)
- `sync` - Sync all package versions without bumping
- `status` - Show current version status
- `tag` - Tag current version and push (triggers CI)
- `release` - Full release: bump patch + tag + push

### CLI Help

```bash
cargo run --bin terminator -- --help     # Show all commands and options
```

### What It Does Automatically

1. **Version bumping** - Semantic versioning (patch/minor/major)
2. **Syncs all packages** - Workspace → Node.js bindings → MCP agent → Platform packages
3. **Git operations** - Auto-commit, tag creation, push to trigger CI
4. **Status checking** - See all versions at a glance

### Version Sync Target

The tool syncs these package versions:

- ✅ **Workspace version** (`Cargo.toml`) - Main source of truth
- ✅ **Node.js bindings** (`bindings/nodejs/package.json`)
- ✅ **MCP agent** (`terminator-mcp-agent/package.json`)
- ✅ **Platform packages** (all `npm/*/package.json` files)

### CI/CD Triggers

The release tool automatically triggers these workflows:

- [publish-npm.yml](.github/workflows/publish-npm.yml) - Node.js packages
- [publish-mcp.yml](.github/workflows/publish-mcp.yml) - MCP agent

## 🛠️ Development Setup

```bash
# Setup
cargo check

# Run examples
cd terminator/examples
cargo run --example basic

# Run tests
cargo test

# Format code (required)
cargo fmt

# Lint (required)
cargo clippy
```

## 📝 Code Style

- **Rust**: Follow `cargo fmt` and fix all `cargo clippy` warnings
- **Documentation**: All public APIs need doc comments with examples
- **Tests**: Add tests for new functionality
- **Examples**: Complex features should have usage examples

## 🧪 Testing

Before submitting PRs:

```bash
cargo fmt && cargo clippy && cargo test
```

## 📦 Adding Dependencies

- Use `workspace = true` for shared dependencies in `Cargo.toml`
- Platform-specific deps should use `#[cfg(target_os = "...")]`
- Minimize external dependencies and justify additions

## 🎥 PR Requirements

This project values video demos! When submitting changes:

- Create a screen recording showing your changes (Cap.so, Screen.studio, etc.)
- All tests must pass
- Documentation updated if needed
- Follow the pull request template

## 💡 Why Rust Release Tool?

✅ **Cross-platform** - Works on Windows, macOS, Linux  
✅ **No external dependencies** - Just `cargo`  
✅ **Type-safe** - Rust prevents runtime errors  
✅ **No shell compatibility issues** - Same syntax everywhere  
✅ **Fast** - Compiled binary vs interpreted scripts
