import asyncio
import terminator
import os

async def run_mspaint():
    desktop = terminator.Desktop(log_level="error")
    try:
        print("Opening Microsoft Paint...")
        paint_window = desktop.open_application("mspaint.exe")
        await asyncio.sleep(2)

        # The following selectors may need adjustment depending on Paint version
        # Try to locate the canvas
        canvas = await paint_window.locator('Name:Canvas').first()
        canvas_bounds = canvas.bounds()
        print(f"Canvas bounds: {canvas_bounds}")

        # Restore original panel, tool_panel, and shapes_toolbar selectors
        panel = paint_window.locator('Pane:UIRibbonDockTop').locator('Pane:Ribbon').locator('Pane:Ribbon').locator('Pane:Ribbon').locator('Pane:Ribbon')
        tool_panel = panel.locator('Pane:Lower Ribbon')
        shapes_toolbar = tool_panel.locator('Name:Shapes')

        # Helper to click the 'More Shapes' button if needed
        async def click_more_shapes_button():
            """
            Click the 'More Shapes' button in the Shapes toolbar if it exists.
            This may be needed to reveal additional shapes in some Paint versions.
            """
            shapes_group = await shapes_toolbar.locator('Group:Shapes').first()
            shapes_group_ele =  shapes_group.explore()
            for child in shapes_group_ele.children:
                if child.role == 'Button' and child.suggested_selector and child.name == 'Shapes':
                    more_shapes_button = await shapes_toolbar.locator(child.suggested_selector).first()
                    more_shapes_button.click()
                    break

        # Helper to select a shape tool by name
        async def select_shape(shape_name):
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
            print(f"Selecting shape tool: {shape_name}")
            await click_more_shapes_button()
            await asyncio.sleep(0.2)
            shapes_box = desktop.locator('window:Shapes').locator('window:Shapes').locator('List:Shapes').locator('Role:Custom')
            tool = shapes_box.locator(f'Name:{shape_name}')
            await tool.click()
            await asyncio.sleep(0.5)

        async def select_brush(brush_name):
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
            print(f"Selecting brush: {brush_name}")
            # Open the Brushes dropdown
            brushes_button = await tool_panel.locator('Name:Brushes').locator('Button:Brushes').first()
            brushes_button.click()
            await asyncio.sleep(0.5)
            brushes_group = paint_window.locator('window:Brushes').locator('List:Brushes').locator('Role:Custom').locator('Group:Brushes')
            brush = await brushes_group.locator(f'Name:{brush_name}').first()
            brush.click()
            await asyncio.sleep(0.5)

        # Draw shapes
        await select_shape('Rounded rectangle')
        canvas.mouse_drag(200, 200, 450, 450)
        await asyncio.sleep(1)

        await select_shape('Triangle')
        canvas.mouse_drag(225, 225, 425, 425)
        await asyncio.sleep(1)

        # Select the pencil tool
        # pencil = await tool_panel.locator('Name:Tools').locator('Name:Pencil').first()
        # pencil.click()

        await select_brush('Calligraphy brush 1')

        # Draw the word TERMINATOR in block letters
        start_x = 460
        start_y = 280
        letter_width = 60
        letter_height = 40
        spacing = 10
        x = start_x
        y = start_y

        # T
        canvas.mouse_drag(x + 0.0 * letter_width, y, x + 1.0 * letter_width, y)
        canvas.mouse_drag(x + 0.5 * letter_width, y, x + 0.5 * letter_width, y + letter_height)
        x += letter_width + spacing

        # E
        canvas.mouse_drag(x, y, x, y + letter_height)
        canvas.mouse_drag(x, y, x + letter_width, y)
        canvas.mouse_drag(x, y + letter_height / 2, x + letter_width * 0.8, y + letter_height / 2)
        canvas.mouse_drag(x, y + letter_height, x + letter_width, y + letter_height)
        x += letter_width + spacing

        # R
        canvas.mouse_drag(x, y, x, y + letter_height)
        canvas.mouse_drag(x, y, x + letter_width * 0.7, y)
        canvas.mouse_drag(x, y + letter_height / 2, x + letter_width * 0.7, y + letter_height / 2)
        canvas.mouse_drag(x + letter_width * 0.7, y, x + letter_width * 0.7, y + letter_height / 2)
        canvas.mouse_drag(x, y + letter_height / 2, x + letter_width, y + letter_height)
        x += letter_width + spacing

        # M
        canvas.mouse_drag(x, y + letter_height, x, y)
        canvas.mouse_drag(x, y, x + letter_width / 2, y + letter_height / 2)
        canvas.mouse_drag(x + letter_width / 2, y + letter_height / 2, x + letter_width, y)
        canvas.mouse_drag(x + letter_width, y, x + letter_width, y + letter_height)
        x += letter_width + spacing

        # I
        canvas.mouse_drag(x + letter_width / 2, y, x + letter_width / 2, y + letter_height)
        x += letter_width + spacing

        # N
        canvas.mouse_drag(x, y + letter_height, x, y)
        canvas.mouse_drag(x, y, x + letter_width, y + letter_height)
        canvas.mouse_drag(x + letter_width, y + letter_height, x + letter_width, y)
        x += letter_width + spacing

        # A
        canvas.mouse_drag(x + letter_width / 2, y, x, y + letter_height)
        canvas.mouse_drag(x + letter_width / 2, y, x + letter_width, y + letter_height)
        canvas.mouse_drag(x + letter_width * 0.25, y + letter_height * 0.6, x + letter_width * 0.75, y + letter_height * 0.6)
        x += letter_width + spacing

        # T
        canvas.mouse_drag(x, y, x + letter_width, y)
        canvas.mouse_drag(x + letter_width / 2, y, x + letter_width / 2, y + letter_height)
        x += letter_width + spacing

        # O
        canvas.mouse_drag(x, y, x + letter_width, y)
        canvas.mouse_drag(x, y + letter_height, x + letter_width, y + letter_height)
        canvas.mouse_drag(x, y, x, y + letter_height)
        canvas.mouse_drag(x + letter_width, y, x + letter_width, y + letter_height)
        x += letter_width + spacing

        # R
        canvas.mouse_drag(x, y, x, y + letter_height)
        canvas.mouse_drag(x, y, x + letter_width * 0.7, y)
        canvas.mouse_drag(x, y + letter_height / 2, x + letter_width * 0.7, y + letter_height / 2)
        canvas.mouse_drag(x + letter_width * 0.7, y, x + letter_width * 0.7, y + letter_height / 2)
        canvas.mouse_drag(x, y + letter_height / 2, x + letter_width, y + letter_height)
        x += letter_width + spacing


        # Open Save As dialog
        print("Opening Save As dialog...")
        paint_window.press_key('{Ctrl}s')
        await asyncio.sleep(1)

        # Enter file name
        print("Entering file name...")
        save_dialog = desktop.locator('window:Save As').locator('window:Save As')
        file_name_edit_box = await save_dialog.locator('role:Pane').locator('role:ComboBox').locator('role:Edit').first()

        home_dir = os.path.expanduser('~')
        file_path = os.path.join(home_dir, 'terminator_paint_test.png')
        file_name_edit_box.type_text(file_path)

        # Find and click the Save button
        save_dialog_ele = await save_dialog.first()
        window_elements = save_dialog_ele.explore()
        for child in window_elements.children:
            if child.role == 'Button' and child.suggested_selector and child.name == 'Save':
                save_button = await save_dialog.locator(child.suggested_selector).first()
                save_button.click()
                break
        print("Save button clicked")

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

        print("File saved successfully!")

    except terminator.PlatformError as e:
        print(f"Platform Error: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")

if __name__ == "__main__":
    asyncio.run(run_mspaint())