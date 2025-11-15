# PyPI Cleanup Workflow

This workflow automatically finds and deletes the oldest release from PyPI using Terminator's browser automation capabilities.

## Usage

### Local Testing

```bash
cd examples/pypi-cleanup-workflow

npm install

npm run build

export PYPI_UI_USERNAME="your_username"
export PYPI_UI_PASSWORD="your_password"
export PYPI_UI_TOTP_SECRET="your_totp_secret"
export PACKAGE_NAME="your-package"

cargo run --bin terminator -- mcp run src/terminator.ts
```
