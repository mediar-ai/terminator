# Software Installer Workflows

This directory contains general-purpose and specific software installation workflows for Terminator MCP. These workflows intelligently attempt CLI-based installation first, then fall back to UI-based installation when necessary.

## Core Workflow

### `install-software.yml`

A flexible, general-purpose software installation workflow that:

1. **Auto-detects package managers** (winget, chocolatey, scoop, brew, apt, etc.)
2. **Attempts CLI installation** using the detected package manager
3. **Downloads installers** from provided URLs if CLI fails
4. **Runs silent installation** with customizable parameters
5. **Falls back to UI automation** for installers without silent mode
6. **Verifies installation** through multiple methods

#### Usage

```bash
terminator mcp run install-software.yml --url http://localhost:3000 \
  --input software_name="7-Zip" \
  --input package_id="7zip.7zip" \
  --input download_url="https://www.7-zip.org/a/7z2301-x64.exe"
```

#### Input Variables

| Variable | Type | Description | Default |
|----------|------|-------------|---------|
| `software_name` | string | Name of the software to install | Required |
| `package_manager` | string | Package manager to use | `auto` |
| `package_id` | string | Package ID for the package manager | Optional |
| `download_url` | string | Direct download URL if package manager fails | Optional |
| `installer_type` | string | Type of installer (msi, exe, dmg, deb, rpm) | `auto` |
| `silent_install_args` | string | Arguments for silent installation | `/quiet /norestart` |
| `ui_install_steps` | array | Custom UI installation steps if CLI fails | Optional |

## Specific Software Workflows

### `install-chrome.yml`

Installs Google Chrome with optional default browser configuration.

```bash
terminator mcp run install-chrome.yml --url http://localhost:3000
```

### `install-vscode.yml`

Installs Visual Studio Code with extensions and configuration.

```bash
terminator mcp run install-vscode.yml --url http://localhost:3000 \
  --input extensions='["ms-python.python", "github.copilot"]'
```

**Features:**
- Installs VS Code via winget or direct download
- Configures default extensions
- Sets up editor preferences
- Enables auto-save and format-on-save

### `install-nodejs.yml`

Installs Node.js with npm, optional Yarn/pnpm, and global packages.

```bash
terminator mcp run install-nodejs.yml --url http://localhost:3000 \
  --input node_version="lts" \
  --input install_yarn=true \
  --input global_packages='["typescript", "nodemon"]'
```

**Features:**
- Supports LTS, latest, or specific versions
- Installs npm packages globally
- Optional Yarn and pnpm installation
- Configures npm for better performance

### `install-git.yml`

Installs Git with comprehensive configuration options.

```bash
terminator mcp run install-git.yml --url http://localhost:3000 \
  --input user_name="John Doe" \
  --input user_email="john@example.com" \
  --input configure_ssh=true
```

**Features:**
- Configures user name and email
- Sets default branch name
- Generates SSH keys for GitHub/GitLab
- Installs Git LFS
- Configures useful Git settings

### `install-python.yml`

Installs Python with pip and virtual environment setup.

```bash
terminator mcp run install-python.yml --url http://localhost:3000 \
  --input python_version="3.11" \
  --input add_to_path=true \
  --input pip_packages='["virtualenv", "black", "pytest"]'
```

**Features:**
- Specific version installation
- PATH configuration
- pip package installation
- Virtual environment setup
- Example project creation

### `install-dev-environment.yml`

Installs a complete development environment with multiple tools.

```bash
terminator mcp run install-dev-environment.yml --url http://localhost:3000 \
  --input install_list='["git", "vscode", "nodejs", "python", "docker"]' \
  --input git_user="John Doe" \
  --input git_email="john@example.com"
```

**Features:**
- Batch installation of multiple tools
- Progress tracking and reporting
- Comprehensive verification
- Customizable software list

## Installation Strategies

The workflows use a multi-layered approach to ensure successful installation:

### 1. Package Manager Detection

Automatically detects and uses the appropriate package manager:

- **Windows**: winget → chocolatey → scoop
- **macOS**: homebrew
- **Linux**: apt → yum → dnf

### 2. Fallback Mechanisms

If the primary method fails, the workflow automatically tries:

1. Direct installer download from official sources
2. Silent installation with appropriate flags
3. UI-based installation with automated clicks
4. Manual installation guidance

### 3. Verification Methods

Each installation is verified through:

- Command execution tests
- Registry checks (Windows)
- File system checks
- PATH availability

## Advanced Usage

### Custom UI Installation Steps

For software with unique installation wizards, provide custom UI steps:

```yaml
ui_install_steps:
  - tool_name: click_element
    arguments:
      selector: "role:Button|name:Custom Install"
  - tool_name: type_into_element
    arguments:
      selector: "role:Edit|name:Installation Path"
      text_to_type: "C:\\CustomPath"
  - tool_name: click_element
    arguments:
      selector: "role:Button|name:Install"
```

### Conditional Installation

Use conditions to control installation flow:

```bash
terminator mcp run install-dev-environment.yml --url http://localhost:3000 \
  --input install_list='["git", "nodejs"]' \
  --start-from-step "install_nodejs"
```

### Dry Run Mode

Test workflows without actual installation:

```bash
terminator mcp run install-software.yml --url http://localhost:3000 \
  --dry-run \
  --input software_name="Test Software"
```

## Error Handling

The workflows include comprehensive error handling:

- **Continues on error** for non-critical steps
- **Timeout protection** for long-running installations
- **Fallback strategies** when primary methods fail
- **Detailed logging** for troubleshooting

## Platform Support

| Platform | CLI Support | UI Support | Tested |
|----------|------------|------------|--------|
| Windows 10/11 | ✅ | ✅ | ✅ |
| macOS | ✅ | ⚠️ | ⚠️ |
| Ubuntu/Debian | ✅ | ⚠️ | ⚠️ |
| RHEL/CentOS | ✅ | ⚠️ | ⚠️ |

⚠️ = Partial support, may require adjustments

## Troubleshooting

### Common Issues

1. **"Package manager not found"**
   - Install winget, chocolatey, or scoop on Windows
   - Install homebrew on macOS
   - Ensure apt/yum is available on Linux

2. **"Installation verification failed"**
   - May require system restart
   - Check if PATH needs updating
   - Verify installation manually

3. **"UI automation failed"**
   - Ensure MCP agent has UI access permissions
   - Check if installer window is in focus
   - Try running with elevated privileges

### Debug Mode

Enable verbose logging:

```bash
LOG_LEVEL=debug terminator mcp run install-software.yml --url http://localhost:3000 \
  --verbose \
  --input software_name="Debug Test"
```

## Contributing

To add support for new software:

1. Create a new workflow file: `install-[software].yml`
2. Use `install-software.yml` as the base
3. Add software-specific configuration
4. Test on target platforms
5. Update this README

## Security Considerations

- **Downloads**: Only use official download URLs
- **Checksums**: Verify installer integrity when possible
- **Permissions**: Run with appropriate privileges
- **Scripts**: Review custom scripts before execution

## Examples

### Install Multiple Tools Sequentially

```bash
# Install essential dev tools
terminator mcp run install-dev-environment.yml --url http://localhost:3000

# Install specific version of Node.js
terminator mcp run install-nodejs.yml --url http://localhost:3000 \
  --input node_version="20.x"

# Install Python with data science packages
terminator mcp run install-python.yml --url http://localhost:3000 \
  --input pip_packages='["numpy", "pandas", "jupyter", "matplotlib"]'
```

### Corporate Environment Setup

```bash
# Install from internal package repository
terminator mcp run install-software.yml --url http://localhost:3000 \
  --input software_name="InternalApp" \
  --input download_url="https://internal.company.com/apps/app.msi" \
  --input silent_install_args="/qn ALLUSERS=1 TARGETDIR=C:\\Apps"
```

## License

These workflows are part of the Terminator MCP project and follow the same license terms.