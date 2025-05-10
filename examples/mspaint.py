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
        def select_shape(shape_name):
            """
            Select a shape tool by its name from the Shapes toolbar.
            Available shapes:
            - Line: Straight line tool
            - Curve: Curved line tool  
            - Oval: Circle/ellipse shape
            - Rectangle: Basic rectangle shape
            - Rounded rectangle: Rectangle with rounded corners
            - Polygon: Multi-sided shape
            - Triangle: Three-sided shape
            - Right-angled triangle: Triangle with 90 degree angle
            - Diamond: Diamond/rhombus shape
            - Pentagon: Five-sided shape
            - Hexagon: Six-sided shape
            - Right arrow: Arrow pointing right
            - Left arrow: Arrow pointing left
            - Up arrow: Arrow pointing up
            - Down arrow: Arrow pointing down
            - Four-point star: Star with 4 points
            - Five-point star: Star with 5 points
            - Six-point star: Star with 6 points
            - Rounded rectangular callout: Speech bubble with rounded rectangle
            - Oval callout: Speech bubble with oval shape
            - Cloud callout: Speech bubble with cloud shape
            - Heart: Heart shape
            - Lightning: Lightning bolt shape
            """
            click_more_shapes_button()
            shapes_box = client.locator('window:Shapes').locator('window:Shapes').locator('List:Shapes').locator('Role:Custom')
            tool = shapes_box.locator(f'Name:{shape_name}')
            print(f"Selecting shape tool: {shape_name}")
            tool.click()

        def select_brush(brush_name):
            """
            Select a brush tool by its name from the Brushes window.
            Available brushes (from exploration):
            - Brush
            - Calligraphy brush 1
            - Calligraphy brush 2
            - Airbrush
            - Oil brush
            - Crayon
            - Marker
            - Natural pencil
            - Watercolour brush
            """
            # Open the Brushes dropdown
            text_tool = tool_panel.locator('Name:Brushes').locator('Button:Brushes')
            text_tool.click()
            time.sleep(0.5)
            # Locate the brushes window and group
            brushes_group = paint_window.locator('window:Brushes').locator('List:Brushes').locator('Role:Custom').locator('Group:Brushes')
            brush = brushes_group.locator(f'Name:{brush_name}')
            print(f"Selecting brush: {brush_name}")
            brush.click()

        # locate the canvas
        canvas = client.locator('Name:Canvas')

        # Get the size of the canvas
        canvas_size = canvas.get_bounds()
        print(f"Canvas size: {canvas_size}")

        select_shape('Rounded rectangle')
        canvas.mouse_drag(200, 200, 450, 450)
        time.sleep(1)

        select_shape('Triangle')
        canvas.mouse_drag(225, 225, 425, 425)
        time.sleep(1)

        # select the pencil tool
        # text_tool = tool_panel.locator('Name:Tools').locator('Name:Pencil')
        # text_tool.click()

        select_brush('Calligraphy brush 1')

        # Draw the word TERMINATOR in block letters
        start_x = 460
        start_y = 280
        letter_width = 60
        letter_height = 40
        spacing = 10
        x = start_x
        y = start_y

        # T
        canvas.mouse_drag(x + 0.0 * letter_width, y, x + 1.0 * letter_width, y)  # top bar
        canvas.mouse_drag(x + 0.5 * letter_width, y, x + 0.5 * letter_width, y + letter_height)  # vertical
        x += letter_width + spacing

        # E
        canvas.mouse_drag(x, y, x, y + letter_height)  # left vertical
        canvas.mouse_drag(x, y, x + letter_width, y)  # top
        canvas.mouse_drag(x, y + letter_height / 2, x + letter_width * 0.8, y + letter_height / 2)  # middle
        canvas.mouse_drag(x, y + letter_height, x + letter_width, y + letter_height)  # bottom
        x += letter_width + spacing

        # R
        canvas.mouse_drag(x, y, x, y + letter_height)  # left vertical
        canvas.mouse_drag(x, y, x + letter_width * 0.7, y)  # top
        canvas.mouse_drag(x, y + letter_height / 2, x + letter_width * 0.7, y + letter_height / 2)  # middle
        canvas.mouse_drag(x + letter_width * 0.7, y, x + letter_width * 0.7, y + letter_height / 2)  # right upper
        canvas.mouse_drag(x, y + letter_height / 2, x + letter_width, y + letter_height)  # diagonal leg
        x += letter_width + spacing

        # M
        canvas.mouse_drag(x, y + letter_height, x, y)  # left vertical
        canvas.mouse_drag(x, y, x + letter_width / 2, y + letter_height / 2)  # left diagonal
        canvas.mouse_drag(x + letter_width / 2, y + letter_height / 2, x + letter_width, y)  # right diagonal
        canvas.mouse_drag(x + letter_width, y, x + letter_width, y + letter_height)  # right vertical
        x += letter_width + spacing

        # I
        canvas.mouse_drag(x + letter_width / 2, y, x + letter_width / 2, y + letter_height)  # vertical
        x += letter_width + spacing

        # N
        canvas.mouse_drag(x, y + letter_height, x, y)  # left vertical
        canvas.mouse_drag(x, y, x + letter_width, y + letter_height)  # diagonal
        canvas.mouse_drag(x + letter_width, y + letter_height, x + letter_width, y)  # right vertical
        x += letter_width + spacing

        # A
        canvas.mouse_drag(x + letter_width / 2, y, x, y + letter_height)  # left diagonal
        canvas.mouse_drag(x + letter_width / 2, y, x + letter_width, y + letter_height)  # right diagonal
        canvas.mouse_drag(x + letter_width * 0.25, y + letter_height * 0.6, x + letter_width * 0.75, y + letter_height * 0.6)  # crossbar
        x += letter_width + spacing

        # T
        canvas.mouse_drag(x, y, x + letter_width, y)  # top bar
        canvas.mouse_drag(x + letter_width / 2, y, x + letter_width / 2, y + letter_height)  # vertical
        x += letter_width + spacing

        # O
        canvas.mouse_drag(x, y, x + letter_width, y)  # top
        canvas.mouse_drag(x, y + letter_height, x + letter_width, y + letter_height)  # bottom
        canvas.mouse_drag(x, y, x, y + letter_height)  # left
        canvas.mouse_drag(x + letter_width, y, x + letter_width, y + letter_height)  # right
        x += letter_width + spacing

        # R
        canvas.mouse_drag(x, y, x, y + letter_height)  # left vertical
        canvas.mouse_drag(x, y, x + letter_width * 0.7, y)  # top
        canvas.mouse_drag(x, y + letter_height / 2, x + letter_width * 0.7, y + letter_height / 2)  # middle
        canvas.mouse_drag(x + letter_width * 0.7, y, x + letter_width * 0.7, y + letter_height / 2)  # right upper
        canvas.mouse_drag(x, y + letter_height / 2, x + letter_width, y + letter_height)  # diagonal leg
        x += letter_width + spacing


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