import logging
import sys
import os
import time

# Add the python-sdk directory to the path to find the terminator_sdk module
SDK_PATH = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'python-sdk'))
if SDK_PATH not in sys.path:
    sys.path.insert(0, SDK_PATH)

# Now we can import the SDK
from desktop_use import DesktopUseClient, ApiError, ConnectionError, sleep

# Configure logging
logging.basicConfig(level=logging.INFO,
                    format='%(levelname)s: %(message)s')

def run_mspaint():
    client = DesktopUseClient()
    try:
        print("Opening Microsoft Paint...")
        client.open_application("mspaint.exe")

        time.sleep(2)  # Wait for Paint to open

        paint_window = client.locator('window:Paint')
        panel = paint_window.locator('Pane:UIRibbonDockTop').locator('Pane:Ribbon').locator('Pane:Ribbon').locator('Pane:Ribbon').locator('Pane:Ribbon')
        tool_panel = panel.locator('Pane:Lower Ribbon')

        # Locate the shapes toolbar
        print("Locating shapes toolbar...")
        shapes_toolbar = tool_panel.locator('Name:Shapes')
        shapes_group = shapes_toolbar.locator('Group:Shapes').explore()

        def click_more_shapes_button():
            for child in shapes_group.children:
                if child.get('role') == 'Button' and child.get('suggested_selector') and child.get('text') == 'Shapes':
                    more_shapes_button = shapes_toolbar.locator('Group:Shapes').locator(child['suggested_selector'])
                    more_shapes_button.click()
                    break

        # Helper to select a shape tool by name
        # Available shapes:
        # - Line: Straight line tool
        # - Curve: Curved line tool  
        # - Oval: Circle/ellipse shape
        # - Rectangle: Basic rectangle shape
        # - Rounded rectangle: Rectangle with rounded corners
        # - Polygon: Multi-sided shape
        # - Triangle: Three-sided shape
        # - Right-angled triangle: Triangle with 90 degree angle
        # - Diamond: Diamond/rhombus shape
        # - Pentagon: Five-sided shape
        # - Hexagon: Six-sided shape
        # - Right arrow: Arrow pointing right
        # - Left arrow: Arrow pointing left
        # - Up arrow: Arrow pointing up
        # - Down arrow: Arrow pointing down
        # - Four-point star: Star with 4 points
        # - Five-point star: Star with 5 points
        # - Six-point star: Star with 6 points
        # - Rounded rectangular callout: Speech bubble with rounded rectangle
        # - Oval callout: Speech bubble with oval shape
        # - Cloud callout: Speech bubble with cloud shape
        # - Heart: Heart shape
        # - Lightning: Lightning bolt shape
        def select_shape(shape_name):
            click_more_shapes_button()
            shapes_box = client.locator('window:Shapes').locator('window:Shapes').locator('List:Shapes').locator('Role:Custom')
            tool = shapes_box.locator(f'Name:{shape_name}')
            print(f"Selecting shape tool: {shape_name}")
            tool.click()

        # locate the canvas
        canvas = client.locator('Name:Canvas')

        # Get the size of the canvas
        canvas_size = canvas.get_bounds()
        print(f"Canvas size: {canvas_size}")

        # Example usage:
        select_shape('Rounded rectangle')
        canvas.mouse_drag(200, 200, 500, 500)
        time.sleep(1)

        select_shape('Triangle')
        canvas.mouse_drag(225, 225, 475, 475)
        time.sleep(1)


        text_tool = tool_panel.locator('Name:Tools').locator('Name:Text')
        text_tool.click()

        canvas.mouse_drag(600,370,600,370)

        paint_window.locator('Name:Text edit box').type_text('Terminator SDK')

        # Open Save As dialog
        print("Opening Save As dialog...")
        paint_window.press_key('{Ctrl}s')

        # Enter file name
        print("Entering file name...")
        save_dialog = client.locator('window:Save As').locator('window:Save As')
        file_name_edit_box = save_dialog.locator('role:Pane').locator('role:ComboBox').locator('role:Edit')

        home_dir = os.path.expanduser('~')
        file_path = os.path.join(home_dir, 'terminator_paint_test.png')
        file_name_edit_box.type_text(file_path)

        # Find and click the Save button
        window_elements = save_dialog.explore()
        for child in window_elements.children:
            if child.get('role') == 'Button' and child.get('suggested_selector') and child.get('text') == 'Save':
                save_button = save_dialog.locator(child['suggested_selector'])
                save_button.click()
                break
        print("Save button clicked")

        # Handle confirmation dialog if file exists
        try:
            confirm_overwrite = save_dialog.explore()
            for child in confirm_overwrite.children:
                if child.get('role') == 'Window' and child.get('suggested_selector') and 'Confirm Save As' in child.get('text'):
                    save_button = save_dialog.locator(child['suggested_selector'])
                    save_button.locator('Name:Yes').click()
                    break
        except:
            pass

        print("File saved successfully!")

    except ApiError as e:
        print(f"API Status: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")

if __name__ == "__main__":
    run_mspaint()