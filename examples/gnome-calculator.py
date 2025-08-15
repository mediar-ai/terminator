import asyncio
import terminator
import subprocess
import re

def get_gnome_calculator_version():
    try:
        output = subprocess.check_output(["gnome-calculator", "--version"], text=True, stderr=subprocess.STDOUT)
        # Output: "gnome-calculator 48.1" or similar
        # Use regex to robustly extract the version number
        match = re.search(r'gnome-calculator\s+([0-9]+(?:\.[0-9]+)*)', output.strip())
        if match:
            version_str = match.group(1)
            major_version = int(version_str.split(".")[0])
            return major_version
        else:
            print(f"Could not parse version from output: {output.strip()}")
            return None
    except Exception as e:
        print(f"Failed to get gnome-calculator version: {e}")
        return None

async def run_calculator():
    desktop = terminator.Desktop(
        log_level="error"
    )  # log_level="error" is used to suppress the info logs
    try:
        print("Opening GNOME Calculator...")
        calculator = desktop.open_application("gnome-calculator")
        await asyncio.sleep(1)

        # Locate the main calculator window or relevant elements
        calc_window = await calculator.locator("role:frame").first()

        button_open_paren = "("
        button_close_paren = ")"
        button_exponent = "Exponent"
        button_sqrt = "√"
        button_pi = "π"
        button_percent = "%"
        button_mod = "mod"
        button_divide = "÷"
        button_multiply = "×"
        button_plus = "+"
        button_minus = "−"
        button_equals = "="
        button_dot = "."
        button_0 = "0"
        button_1 = "1"
        button_2 = "2"
        button_3 = "3"
        button_4 = "4"
        button_5 = "5"
        button_6 = "6"
        button_7 = "7"
        button_8 = "8"
        button_9 = "9"

        print("Clicking buttons to perform a calculation...")

        # Simple calculation: 7 + 8 - 4 × 2 + 6 ÷ 3 =
        button_labels = [
            button_7,
            button_plus,
            button_8,
            button_minus,
            button_4,
            button_multiply,
            button_2,
            button_plus,
            button_6,
            button_divide,
            button_3,
            button_equals,
        ]
        for label in button_labels:
            button = await calc_window.locator(f"button:{label}").first()
            # Note: Using perform_action() instead of click() for cross-platform compatibility
            # - click() requires exact coordinates which are only available on X11
            # - On Wayland, ATSPI coordinates are invalid due to security restrictions
            # - perform_action() uses platform-agnostic ATSPI actions that work reliably
            button.perform_action("click")
            print(f"Clicked {label}")

        print("Retrieving result...")
        await asyncio.sleep(1)

        version = get_gnome_calculator_version()
        print(f"GNOME Calculator version: {version}")
        if version == 48:
            # In GNOME Calculator 48, the result is shown as a list of labels inside a list item
            result_field = await calc_window.locator("role:list item").locator("role:label").all()
            result = ""
            for child in result_field:
                result += child.text() + " "
            result = result.strip()
        else:
            # Fallback for older versions: use the editbar
            result_field = await calc_window.locator("role:editbar").first()
            result = result_field.text()
        print(f"Calculation result: {result}")

    except terminator.PlatformError as e:
        print(f"Platform Error: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")


if __name__ == "__main__":
    asyncio.run(run_calculator())
