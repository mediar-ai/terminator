# `pip install -e .`
# or `maturin develop`

import asyncio
import terminator

async def main():
    desktop = terminator.PyDesktop()
    print(desktop.hello())
    print("Root element:", desktop.root().role, desktop.root().name)

    # List applications
    apps = desktop.applications()
    print("Applications:", [(a.role, a.name) for a in apps])

    # Find an element using a selector (async)
    locator = desktop.locator("role:button")
    try:
        button = await locator.first()
        print("Found button:", button.role, button.name)
        button.click()
    except Exception as e:
        print("No button found or click failed:", e)

    # Run a command (async)
    cmd = await desktop.run_command(windows_command="echo hello", unix_command="echo hello")
    print("Command output:", cmd.stdout)

    # Screenshot (async)
    screenshot = await desktop.capture_screen()
    print(f"Screenshot: {screenshot.width}x{screenshot.height}, {len(screenshot.image_data)} bytes")

    # Error handling example
    try:
        not_found = desktop.application("NonExistentApp")
    except Exception as e:
        print("Expected error:", e)

    # Show docstring help
    print("\nHelp for PyDesktop:")
    help(terminator.PyDesktop)

if __name__ == "__main__":
    asyncio.run(main()) 