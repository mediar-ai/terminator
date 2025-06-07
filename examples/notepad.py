import asyncio
import terminator
import os
import platform

async def run_notepad():
    desktop = terminator.Desktop(log_level="error") # log_level="error" is used to suppress the info logs
    try:
        print("Opening Notepad...")
        editor = desktop.open_application("notepad.exe")
        await asyncio.sleep(2)

        editor.highlight(duration_ms=5000)  # Red color (Default) for 5 seconds

        if platform.release() == "11":
            document = await editor.locator('role:Document').first()
            document.highlight(color=0x00FF00, duration_ms=2000)  # Green color for 2 seconds
            AddButton = await editor.locator('name:Add New Tab').first()
            AddButton.highlight(color=0x0000FF, duration_ms=2000)  # Blue color for 2 seconds
            AddButton.click()
        else:
            document = await editor.locator('role:Edit').first()
            document.highlight(color=0x00FF00, duration_ms=2000)  # Green color for 2 seconds

        print('typing text...')
        document.type_text('hello from terminator!\nthis is a python test.')
        await asyncio.sleep(1)

        print('pressing enter...')
        document.press_key('{Enter}')
        await asyncio.sleep(1)

        document.type_text('done.')

        content = document.text()
        # Process the text to handle various line endings robustly
        lines = content.splitlines()
        cleaned_text = '\n'.join(lines)
        print(f'notepad content retrieved: {cleaned_text}')

        print("Opening Save As dialog...")
        document.press_key('{Ctrl}s')

        print("Entering file name...")
        save_dialog = desktop.locator('window:Save As').locator('window:Save As')
        save_dialog_window = await save_dialog.first()
        save_dialog_window.highlight(color=0xFF00FF, duration_ms=3000)  # Magenta color for 3 seconds
        await asyncio.sleep(1)
        file_name_edit_box = await save_dialog.locator('role:Pane').locator('role:ComboBox').locator('role:Edit').first()
        file_name_edit_box.highlight(color=0xFFFF00, duration_ms=3000)  # Yellow color for 3 seconds

        home_dir = os.path.expanduser('~')
        file_path = os.path.join(home_dir, 'terminator_notepad_test.md')
        file_name_edit_box.type_text(file_path)
        
        # Get the pane and explore its contents
        pane = await save_dialog.locator('role:Pane').first()
        pane.highlight(color=0x00FFFF, duration_ms=3000)  # Cyan color for 3 seconds
        pane_elements = pane.explore()
        
        # Find and click the Save as type ComboBox
        # This changes the file type to `All Files` so that we can save it in any file format
        for child in pane_elements.children:
            if child.role == 'ComboBox' and child.suggested_selector and child.name == 'Save as type:':
                combo_box = await save_dialog.locator(child.suggested_selector).first()
                combo_box.highlight(color=0xFFA500, duration_ms=2000)  # Orange color for 2 seconds
                combo_box.click()
                combo_box.press_key('{Ctrl}a')
                break
        
        # Find and click the Save button
        save_dialog_ele = await save_dialog.first()
        window_elements = save_dialog_ele.explore()
        for child in window_elements.children:
            if child.role == 'Button' and child.suggested_selector and child.name == 'Save':
                save_button = await save_dialog.locator(child.suggested_selector).first()
                save_button.highlight(color=0x800080, duration_ms=2000)  # Purple color for 2 seconds
                save_button.click()
                break

        # Handle the confirmation dialog if file exists
        try:
            save_dialog_ele = await save_dialog.first()
            confirm_overwrite = save_dialog_ele.explore()
            for child in confirm_overwrite.children:
                if child.role == 'Window' and child.suggested_selector and 'Confirm Save As' in child.text:
                    save_button = await save_dialog.locator(child.suggested_selector).locator('Name:Yes').first()
                    save_button.click()
                    break
        except:
            pass

    except terminator.PlatformError as e:
        print(f"Platform Error: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")

if __name__ == "__main__":
    asyncio.run(run_notepad())