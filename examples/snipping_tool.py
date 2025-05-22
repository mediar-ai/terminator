import logging
import sys
import os
import time
import math
from enum import Enum

# Add the python-sdk directory to the path to find the terminator_sdk module
SDK_PATH = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'python-sdk'))
if SDK_PATH not in sys.path:
    sys.path.insert(0, SDK_PATH)

# Now we can import the SDK
from desktop_use import DesktopUseClient, ApiError, ConnectionError, sleep

logging.basicConfig(level=logging.INFO,
                    format='%(levelname)s: %(message)s')

def run_snipping_tool():
    client = DesktopUseClient()
    try:
        print("Opening Snipping Tool...")
        client.open_application("SnippingTool.exe")
        time.sleep(2)  # Wait for Snipping Tool to open

        app_window = client.locator('window:Snipping Tool')

        class SnipMode(Enum):
            FREEFORM = "Free-form Snip"
            RECTANGULAR = "Rectangular Snip"
            WINDOW = "Window Snip"
            FULL_SCREEN = "Full-screen Snip"

        def select_snip_mode(mode: SnipMode) -> None:
            print(f"Selecting snip mode: {mode.value}")
            app_window.locator('SplitButton:Mode').click()
            menu = client.locator('Menu:Context')
            menu.locator(f'Name:{mode.value}').click()

        def draw_polygon(app_window, center_x, center_y, radius, sides=6, rounds=1, sleep_time=0.01):
            if sides < 3:
                raise ValueError("Polygon must have at least 3 sides.")
            angle0 = 0
            x0 = center_x + radius * math.cos(angle0)
            y0 = center_y + radius * math.sin(angle0)
            app_window.mouse_click_and_hold(x0, y0)
            time.sleep(sleep_time)
            for r in range(rounds):
                for i in range(1, sides + 1):
                    angle = 2 * math.pi * i / sides
                    x = center_x + radius * math.cos(angle)
                    y = center_y + radius * math.sin(angle)
                    app_window.mouse_move(x, y)
                    if sleep_time > 0:
                        time.sleep(sleep_time)
            app_window.mouse_release()

        select_snip_mode(SnipMode.FREEFORM)
        time.sleep(1)
        N = 100  # Number of sides for a near-circle
        draw_polygon(app_window, 300, 300, 200, N, 1, 0.01)
        print("Free-form snip drawn!")

        time.sleep(1)

        print("Opening Save As dialog...")
        app_window.press_key('{Ctrl}s')

        time.sleep(1)

        print("Entering file name...")
        save_dialog = client.locator('window:Save As').locator('window:Save As')
        file_name_edit_box = save_dialog.locator('role:Pane').locator('role:ComboBox').locator('role:Edit')

        time.sleep(1)

        home_dir = os.path.expanduser('~')
        file_path = os.path.join(home_dir, 'terminator_snip_test.png')
        file_name_edit_box.type_text(file_path)

        # Find and click the Save button
        window_elements = client.locator('Name:Save As').explore()
        for child in window_elements.children:
            if child.get('role') == 'Button' and child.get('suggested_selector') and child.get('name') == 'Save':
                save_button = save_dialog.locator(child['suggested_selector'])
                save_button.click()
                break

        # This is a workaround to handle the confirmation dialog that appears when saving a file that already exists
        confirm_overwrite = client.locator('role:Window').locator('Name:Save As').explore()
        for child in confirm_overwrite.children:
            if child.get('role') == 'Window' and child.get('suggested_selector') and 'Confirm Save As' in child.get('name'):
                save_button = save_dialog.locator(child['suggested_selector'])
                save_button.locator('Name:Yes').click()
                break

        print("File saved successfully!")

    except ApiError as e:
        print(f"API Status: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")

if __name__ == "__main__":
    run_snipping_tool() 