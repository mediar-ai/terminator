import asyncio
import terminator
import os
import math
from enum import Enum
import platform

desktop = terminator.Desktop(log_level="error")


def is_windows_11():
    version = platform.version()
    return platform.system() == "Windows" and int(version.split(".")[2]) >= 22000


class SnipMode(Enum):
    FREEFORM = ("Freeform", "Free-form Snip")
    RECTANGULAR = ("Rectangle", "Rectangular Snip")
    WINDOW = ("Window", "Window Snip")
    FULL_SCREEN = ("Full screen", "Full-screen Snip")

    def __init__(self, win11_label, win10_label):
        self._win11_label = win11_label
        self._win10_label = win10_label

    @property
    def value(self):
        return self._win11_label if is_windows_11() else self._win10_label


async def select_snip_mode(app_window: terminator.Locator, mode: SnipMode):
    print(f"Selecting snip mode: {mode.value}")
    if is_windows_11():
        button = await app_window.locator("ComboBox:Snipping Mode").first()
        options = button.list_options()
        for option in options:
            if option == mode.value:
                button.select_option(option)
                break
    else:
        button = await app_window.locator("SplitButton:Mode").first()
        button.click()
        menu = desktop.locator("Menu:Context")
        mode_button = await menu.locator(f"Name:{mode.value}").first()
        mode_button.click()


async def draw_polygon(
    app_window: terminator.UIElement,
    center_x,
    center_y,
    radius,
    sides=6,
    rounds=1,
    sleep_time=0.01,
):
    if sides < 3:
        raise ValueError("Polygon must have at least 3 sides.")
    angle0 = 0
    x0 = center_x + radius * math.cos(angle0)
    y0 = center_y + radius * math.sin(angle0)
    app_window.mouse_click_and_hold(x0, y0)
    await asyncio.sleep(sleep_time)
    for r in range(rounds):
        for i in range(1, sides + 1):
            angle = 2 * math.pi * i / sides
            x = center_x + radius * math.cos(angle)
            y = center_y + radius * math.sin(angle)
            app_window.mouse_move(x, y)
            if sleep_time > 0:
                await asyncio.sleep(sleep_time)
    app_window.mouse_release()


async def run_snipping_tool():
    try:
        print("Opening Snipping Tool...")
        app = desktop.open_application("SnippingTool.exe")

        app_window: terminator.Locator = desktop.locator("window:Snipping Tool")

        if is_windows_11():
            toggle = await app_window.locator("Name:Capture mode").first()
            if toggle.name() != "Capture mode set to snipping":
                toggle.set_toggled(not toggle.is_toggled())

        await select_snip_mode(app_window, SnipMode.FREEFORM)
        await asyncio.sleep(1)

        if is_windows_11():
            new_screenshot_button = await app_window.locator(
                "Name:New screenshot"
            ).first()
            new_screenshot_button.click()

        N = 100  # Number of sides for a near-circle
        screen: terminator.UIElement = await app_window.first()
        await draw_polygon(screen, 300, 300, 200, N, 1, 0.01)
        print("Free-form snip drawn!")

        await asyncio.sleep(1)

        print("Opening Save As dialog...")
        window = await app_window.first()
        window.press_key("{Ctrl}s")
        await asyncio.sleep(1)

        print("Entering file name...")
        save_dialog = app_window.locator("window:Save As")
        file_name_edit_box = (
            await save_dialog.locator("role:Pane")
            .locator("role:ComboBox")
            .locator("role:Edit")
            .first()
        )

        home_dir = os.path.expanduser("~")
        file_path = os.path.join(home_dir, "terminator_snip_test.png")
        file_name_edit_box.type_text(file_path)
        file_already_exists = os.path.exists(file_path)

        # Find and click the Save button
        save_button = await save_dialog.locator("Button:Save").first()
        save_button.click()

        print("save button clicked")

        # Handle the confirmation dialog if file exists
        if file_already_exists:
            confirm_overwrite = (
                await save_dialog.locator("Window:Confirm Save As")
                .locator("Name:Yes")
                .first()
            )
            confirm_overwrite.click()
            print("confirm overwrite clicked")

        print("File saved successfully!")

        print("Closing Snipping Tool...")
        app = await app_window.first()
        app.close()

    except terminator.PlatformError as e:
        print(f"Platform Error: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")


if __name__ == "__main__":
    asyncio.run(run_snipping_tool())
