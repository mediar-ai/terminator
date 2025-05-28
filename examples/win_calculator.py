# examples/win_calculator.py
# Import necessary modules
import asyncio
import terminator


async def find_calculator_results(calculator_window):
    """
    Find the element with AutomationId 'CalculatorResults' within the Group element.
    :param calculator_window: The calculator window locator
    :return: A locator for the CalculatorResults element or None if not found
    """
    # Get the display element and explore result
    display_element = calculator_window.locator("Id:CalculatorResults")
    explore_result = await display_element.explore()
    
    # Find the Group element
    for child in explore_result.children:
        if child.role == 'Group' and child.suggested_selector:
            # Get the Group element's children
            group_result = await calculator_window.locator(child.suggested_selector).explore()
            # Search for CalculatorResults within the Group's children
            for group_child in group_result.children:
                if group_child.suggested_selector:
                    child_locator = calculator_window.locator(group_child.suggested_selector)
                    child_attrs = await child_locator.attributes()
                    if 'CalculatorResults' in str(child_attrs.properties.get("AutomationId")):
                        return child_locator
    return None

async def run_calculator():
    # Create a Desktop instance (main entry point for automation)
    desktop = terminator.Desktop(log_level="error")
    try:
        # 1. Open Calculator
        print("Opening Calculator...")
        desktop.open_application("uwp:Microsoft.WindowsCalculator")
        await asyncio.sleep(2)  # Allow app to open

        # 2. Create locators for UI elements
        calculator_window = desktop.locator("window:Calculator")  # Adjust selector if window title is different
        # Locators relative to the calculator window
        # IMPORTANT: Selectors might differ on non-Windows platforms or even Windows versions
        display_element = calculator_window.locator("Id:CalculatorResults")  # Using AutomationId is often more stable
        button_1 = calculator_window.locator("Name:One")
        button_plus = calculator_window.locator("Name:Plus")
        button_2 = calculator_window.locator("Name:Two")
        button_equals = calculator_window.locator("Name:Equals")

        # 3. Get initial display text
        print("Getting initial display text...")
        try:
            # Get the attributes of the display element
            display_element_attributes = await display_element.attributes()
            # Determine if we need to find CalculatorResults
            needs_calculator_results = 'CalculatorResults' not in str(display_element_attributes.properties.get("AutomationId"))
            # Only proceed if not already CalculatorResults
            if needs_calculator_results:
                print("Finding CalculatorResults element...")
                display_element = await find_calculator_results(calculator_window)
                if display_element:
                    text = (await display_element.name())
                    print(f"Text: {text}")
                else:
                    print("Could not find element with AutomationId 'CalculatorResults'")
            else:
                print("Element already has AutomationId 'CalculatorResults'")
        except Exception as e:
            print(f"Warning: Could not get initial display text: {e}")

        # 4. Perform clicks (1 + 2 =)
        print("Performing clicks: 1 + 2 =")
        await button_1.click()
        await asyncio.sleep(0.5)
        await button_plus.click()
        await asyncio.sleep(0.5)
        await button_2.click()
        await asyncio.sleep(0.5)
        await button_equals.click()
        await asyncio.sleep(1.0)  # Wait for calculation

        # 5. Get final text & verify
        print("Verifying final text (expecting 3)...")
        try:
            # If we needed to find CalculatorResults earlier, find it again as it might be unstable
            if needs_calculator_results:
                display_element = await find_calculator_results(calculator_window)
                if not display_element:
                    print("Could not find CalculatorResults element for verification")
                    return
            # Get the text and verify it
            text_response = await display_element.name()
            if text_response == "Display is 3":
                print("Final display text is verified to be 'Display is 3'")
            elif text_response == "3":
                print("Final display text is verified to be '3'")
            else:
                print(f"Unexpected text: {text_response}")
            # Optionally get text again after verification
            final_text = await display_element.name()
            print(f"Final display text (raw): {final_text}")
        except Exception as e:
            print(f"Verification failed or could not get final text: {e}")
            # Try getting raw text anyway on failure for debugging
            try:
                raw_text = await display_element.name()
                print(f"Raw text on failure: {raw_text}")
            except Exception as inner_e:
                print(f"Could not get raw text after verification failure: {inner_e}")

        # Example: Get attributes of the equals button
        print("Getting attributes of '=' button...")
        attrs = await button_equals.attributes()
        print(f"Equals button attributes: {attrs}")

        # Example: Check visibility of the equals button
        print("Checking visibility of '=' button...")
        is_visible = await button_equals.is_visible()
        print(f"Is Equals button visible? {is_visible}")

    except terminator.PlatformError as e:
        print(f"Platform Error: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")

# Entry point for running the example
if __name__ == "__main__":
    asyncio.run(run_calculator()) 