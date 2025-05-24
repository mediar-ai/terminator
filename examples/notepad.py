import logging
import sys
import os
import time
import platform

# Add the python-sdk directory to the path to find the terminator_sdk module
SDK_PATH = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'python-sdk'))
if SDK_PATH not in sys.path:
    sys.path.insert(0, SDK_PATH)

# Now we can import the SDK
from desktop_use import DesktopUseClient, ApiError, ConnectionError, sleep

# --- Configuration --- #
# Optional: Configure logging for more detailed output
# logging.basicConfig(level=logging.DEBUG,
#                     format='%(asctime)s - %(name)s - %(levelname)s - %(message)s')
logging.basicConfig(level=logging.INFO,
                    format='%(levelname)s: %(message)s')

# Ensure the Terminator server (e.g., `cargo run --example server`) is running!

def run_notepad():
    client = DesktopUseClient()
    try:
        print("Opening Notepad...")
        client.open_application("notepad.exe")
        time.sleep(2)  # Wait for Notepad to open

        editor = client.locator('window:Notepad')
        editor.highlight(duration_ms=5000)  # Red color (Default) for 2 seconds
        document = editor.locator('role:Document')
        document.highlight(color=0x00FF00, duration_ms=2000)  # Green color for 2 seconds

        if platform.release() == "11":
            AddButton = editor.locator('name:Add New Tab')
            AddButton.highlight(color=0x0000FF, duration_ms=2000)  # Blue color for 2 seconds
            AddButton.click()

        print('typing text...')
        document.type_text('hello from terminator!\nthis is a python test.')
        time.sleep(1)

        print('pressing enter...')
        document.press_key('{Enter}')
        time.sleep(1)

        document.type_text('done.')

        content = document.get_text()
        # Process the text to handle various line endings robustly
        lines = content.text.splitlines()
        cleaned_text = '\n'.join(lines)
        print(f'notepad content retrieved:\n{cleaned_text}')

        print("Opening Save As dialog...")
        document.press_key('{Ctrl}s')

        print("Entering file name...")
        save_dialog = client.locator('window:Save As').locator('window:Save As')
        save_dialog.highlight(color=0xFF00FF, duration_ms=3000)  # Magenta color for 3 seconds
        time.sleep(1)
        file_name_edit_box = save_dialog.locator('role:Pane').locator('role:ComboBox').locator('role:Edit')
        file_name_edit_box.highlight(color=0xFFFF00, duration_ms=3000)  # Yellow color for 3 seconds

        home_dir = os.path.expanduser('~')
        file_path = os.path.join(home_dir, 'terminator_notepad_test.md')
        file_name_edit_box.type_text(file_path)
        
        # Get the pane and explore its contents
        pane = save_dialog.locator('role:Pane')
        pane.highlight(color=0x00FFFF, duration_ms=3000)  # Cyan color for 3 seconds
        pane_elements = pane.explore()
        
        # Find and click the Save as type ComboBox
        # This changes the file type to `All Files` so that we can save it in any file format
        for child in pane_elements.children:
            if child.get('role') == 'ComboBox' and child.get('suggested_selector') and child.get('name') == 'Save as type:':
                combo_box = save_dialog.locator(child['suggested_selector'])
                combo_box.highlight(color=0xFFA500, duration_ms=2000)  # Orange color for 2 seconds
                combo_box.click()
                combo_box.press_key('{Ctrl}a')
                break
        
        # Find and click the Save button
        window_elements = save_dialog.explore()
        for child in window_elements.children:
            if child.get('role') == 'Button' and child.get('suggested_selector') and child.get('name') == 'Save':
                save_button = save_dialog.locator(child['suggested_selector'])
                save_button.highlight(color=0x800080, duration_ms=2000)  # Purple color for 2 seconds
                save_button.click()
                break

        # This is a workaround to handle the confirmation dialog that appears when saving a file that already exists
        confirm_overwrite = save_dialog.explore()
        for child in confirm_overwrite.children:
            if child.get('role') == 'Window' and child.get('suggested_selector') and 'Confirm Save As' in child.get('name'):
                save_button = save_dialog.locator(child['suggested_selector'])
                save_button.highlight(color=0x008080)  # Teal color
                save_button.locator('Name:Yes').click()
                break

        print("File saved successfully!")

    except ApiError as e:
        print(f"API Status: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")

if __name__ == "__main__":
    run_notepad()