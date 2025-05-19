# Example: Windows Login Automator

This example demonstrates how to automate filling a simple login form using the Terminator Python SDK.

The `dummy_login.py` script opens a minimal login window with username and password fields. The `login_automator.py` script launches this window via the Terminator server and fills in the credentials automatically.

## Prerequisites

1. **Terminator Server** running on your machine. See the [project README](../../README.md#quick-start) for setup instructions.
2. **Python** installed (the scripts use `sys.executable` to run the login window).

## Running the example

```bash
# From the repository root
cd examples/login-automator
python login_automator.py
```

The automation script will:
1. Launch `dummy_login.py` via the Terminator server.
2. Locate the login window titled **"Dummy Login"**.
3. Type the `DEMO_USERNAME` and `DEMO_PASSWORD` environment variables (or default values) into the form.
4. Click the **Login** button.

Adjust selectors in `login_automator.py` if your accessibility tree differs.
