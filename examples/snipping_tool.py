import asyncio
import terminator
import os
import math
from enum import Enum

desktop = terminator.Desktop(log_level="error")

class SnipMode(Enum):
    FREEFORM = "Free-form Snip"
    RECTANGULAR = "Rectangular Snip"
    WINDOW = "Window Snip"
    FULL_SCREEN = "Full-screen Snip"

async def select_snip_mode(app_window, mode: SnipMode):
    print(f"Selecting snip mode: {mode.value}")
    await app_window.locator('SplitButton:Mode').click()
    menu = desktop.locator('Menu:Context')
    await menu.locator(f'Name:{mode.value}').click()

async def draw_polygon(app_window, center_x, center_y, radius, sides=6, rounds=1, sleep_time=0.01):
    if sides < 3:
        raise ValueError("Polygon must have at least 3 sides.")
    angle0 = 0
    x0 = center_x + radius * math.cos(angle0)
    y0 = center_y + radius * math.sin(angle0)
    await app_window.mouse_click_and_hold(x0, y0)
    await asyncio.sleep(sleep_time)
    for r in range(rounds):
        for i in range(1, sides + 1):
            angle = 2 * math.pi * i / sides
            x = center_x + radius * math.cos(angle)
            y = center_y + radius * math.sin(angle)
            await app_window.mouse_move(x, y)
            if sleep_time > 0:
                await asyncio.sleep(sleep_time)
    await app_window.mouse_release()

async def run_snipping_tool():
    try:
        print("Opening Snipping Tool...")
        desktop.open_application("SnippingTool.exe")
        await asyncio.sleep(2)

        app_window = desktop.locator('window:Snipping Tool')

        await select_snip_mode(app_window, SnipMode.FREEFORM)
        await asyncio.sleep(1)
        N = 100  # Number of sides for a near-circle
        await draw_polygon(app_window, 300, 300, 200, N, 1, 0.01)
        print("Free-form snip drawn!")

        await asyncio.sleep(1)

        print("Opening Save As dialog...")
        await app_window.press_key('{Ctrl}s')
        await asyncio.sleep(1)

        print("Entering file name...")
        save_dialog = desktop.locator('window:Save As').locator('window:Save As')
        file_name_edit_box = save_dialog.locator('role:Pane').locator('role:ComboBox').locator('role:Edit')

        home_dir = os.path.expanduser('~')
        file_path = os.path.join(home_dir, 'terminator_snip_test.png')
        await file_name_edit_box.type_text(file_path)

        # Find and click the Save button
        window_elements = await save_dialog.explore()
        for child in window_elements.children:
            if child.role == 'Button' and child.suggested_selector and child.name == 'Save':
                save_button = save_dialog.locator(child.suggested_selector)
                await save_button.click()
                break

        # Handle the confirmation dialog if file exists
        confirm_overwrite = await save_dialog.explore()
        for child in confirm_overwrite.children:
            if child.role == 'Window' and child.suggested_selector and 'Confirm Save As' in child.text:
                save_button = save_dialog.locator(child.suggested_selector)
                await save_button.locator('Name:Yes').click()
                break

        print("File saved successfully!")

    except terminator.PlatformError as e:
        print(f"Platform Error: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")

if __name__ == "__main__":
    asyncio.run(run_snipping_tool()) 