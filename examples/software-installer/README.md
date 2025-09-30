# Universal Software Installer

A single, simple workflow that installs any software by name. Just pass the software name and it handles everything.

## Usage

```bash
# Install Chrome
terminator mcp run universal-installer.yml --url http://localhost:3000 --input software="chrome"

# Install VS Code
terminator mcp run universal-installer.yml --url http://localhost:3000 --input software="vscode"

# Install Discord
terminator mcp run universal-installer.yml --url http://localhost:3000 --input software="discord"

# Install OneDrive
terminator mcp run universal-installer.yml --url http://localhost:3000 --input software="onedrive"
```

## How It Works

1. **Maps software name** to package IDs and download URLs
2. **Tries CLI installation** via winget → chocolatey → scoop
3. **Downloads installer** if CLI fails
4. **Runs silent installation** with appropriate flags
5. **Falls back to browser** for manual download if needed

## Supported Software

The workflow has built-in support for:

- **Browsers**: chrome, firefox, brave
- **Development**: vscode, git, nodejs, python, docker, postman
- **Communication**: discord, slack, zoom, teams
- **Media**: spotify, vlc, obs
- **Utilities**: 7zip, notepad++, steam
- **Microsoft**: onedrive, teams

For any other software, it will:
1. Try common package manager naming conventions
2. Fall back to browser search for manual download

## Features

- ✅ Single workflow for all software
- ✅ Automatic package manager detection
- ✅ Silent installation support
- ✅ Fallback to UI when needed
- ✅ Cross-platform package manager support
- ✅ No complex configuration needed

## Examples

```bash
# Install multiple software sequentially
terminator mcp run universal-installer.yml --url http://localhost:3000 --input software="git"
terminator mcp run universal-installer.yml --url http://localhost:3000 --input software="nodejs"
terminator mcp run universal-installer.yml --url http://localhost:3000 --input software="vscode"

# Install communication tools
terminator mcp run universal-installer.yml --url http://localhost:3000 --input software="discord"
terminator mcp run universal-installer.yml --url http://localhost:3000 --input software="slack"
```

## Adding New Software

To add support for new software, edit the `softwareMap` in the workflow:

```javascript
'newsoftware': {
  name: 'Software Display Name',
  winget: 'Publisher.SoftwareName',
  choco: 'package-name',
  download: 'https://download.url/installer.exe',
  silent: '/S'
}
```

That's it! One workflow, any software.