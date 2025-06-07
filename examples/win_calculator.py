# examples/win_calculator.py
# Import necessary modules
import asyncio
import terminator

async def run_calculator():
    # Create a Desktop instance (main entry point for automation)
    desktop = terminator.Desktop(log_level="error")
    try:
        # 1. Open Calculator
        print("Opening Calculator...")
        calculator_window = desktop.open_application("uwp:Microsoft.WindowsCalculator")
        await asyncio.sleep(2)  # Allow app to open

        # Locators relative to the calculator window
        # IMPORTANT: Selectors might differ on non-Windows platforms or even Windows versions
        display_element = calculator_window.locator("nativeid:CalculatorResults")  # Using AutomationId is often more stable
        button_1 = await calculator_window.locator("Name:One").first()
        button_plus = await calculator_window.locator("Name:Plus").first()
        button_2 = await calculator_window.locator("Name:Two").first()
        button_equals = await calculator_window.locator("Name:Equals").first()

        # 3. Get initial display text
        print("Getting initial display text...")
        try:
            element = await display_element.first()
            text = element.name()
            print(f"Text: {text}")
        except Exception as e:
            print(f"Warning: Could not get initial display text: {e}")

        # 4. Perform clicks (1 + 2 =)
        print("Performing clicks: 1 + 2 =")

        button_1.click()
        await asyncio.sleep(0.5)

        button_plus.click()
        await asyncio.sleep(0.5)

        button_2.click()
        await asyncio.sleep(0.5)

        button_equals.click()
        await asyncio.sleep(1.0)  # Wait for calculation

        # 5. Get final text & verify
        print("Verifying final text (expecting 3)...")
        try:
            # Get the text and verify it
            element = await display_element.first()
            text_response = element.name()
            if text_response == "Display is 3":
                print("Final display text is verified to be 'Display is 3'")
            elif text_response == "3":
                print("Final display text is verified to be '3'")
            else:
                print(f"Unexpected text: {text_response}")
        except Exception as e:
            print(f"Verification failed or could not get final text: {e}")

        # Example: Get attributes of the equals button
        print("Getting attributes of '=' button...")
        attrs = button_equals.attributes()
        print(f"Equals button attributes: {attrs}")

        # Example: Check visibility of the equals button
        print("Checking visibility of '=' button...")
        is_visible = button_equals.is_visible()
        print(f"Is Equals button visible? {is_visible}")

    except terminator.PlatformError as e:
        print(f"Platform Error: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")

# Entry point for running the example
if __name__ == "__main__":
    asyncio.run(run_calculator()) 