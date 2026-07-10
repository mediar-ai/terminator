"""
Element Screenshot Example

Launches Notepad, types several lines of text, then captures a screenshot of the
editor body, frames it with a border, and runs OCR on it -- a more interesting
demo than grabbing the first anonymous button on the desktop. The editor body is
large and high-contrast, so OCR reliably reads the text back.

Requirements:
    pip install Pillow
"""

import asyncio
import os
import platform
import subprocess
import sys
try:
    from PIL import Image, ImageOps
except ImportError:
    print("Please install Pillow: pip install Pillow")
    exit(1)
import terminator

# Multi-line sample so the OCR result is interesting.
SAMPLE_TEXT = (
    "hello from terminator!\n"
    "this is a python test.\n"
    "the editor body is captured and OCR'd."
)

# Launch Notepad by its full path rather than the bare name "notepad". On some
# machines "notepad(.exe)" is remapped to a third-party editor (e.g. Notepad++),
# and open_application() would hand us that window instead.
NOTEPAD_PATH = r"C:\Windows\System32\notepad.exe"

# Bind to the freshly launched Notepad window by its title. open_application()
# can return a sibling window handle that doesn't contain the editor, so we
# locate the real one explicitly. "Untitled - Notepad" also avoids matching a
# "... - Notepad++" window, which a bare "Notepad" substring would catch.
NOTEPAD_WINDOW = "role:Window|name:contains:Untitled - Notepad"

# Border drawn around the captured element (RGBA) and its thickness in pixels.
BORDER_COLOR = (0, 255, 0, 255)  # green
BORDER_WIDTH = 6


def open_image(path):
    """Open an image file in the OS default viewer (Windows/macOS/Linux)."""
    path = os.path.abspath(path)
    print(f"Opening {path} ...")
    try:
        if sys.platform.startswith("win"):
            os.startfile(path)  # type: ignore[attr-defined]
        elif sys.platform == "darwin":
            subprocess.run(["open", path], check=False)
        else:
            subprocess.run(["xdg-open", path], check=False)
    except Exception as e:
        print(f"Could not open {path}: {e}")


async def capture_editor_body(desktop: terminator.Desktop):
    """Open Notepad, type text, and capture + OCR the editor body."""
    print("Opening Notepad...")
    desktop.open_application(NOTEPAD_PATH)
    await asyncio.sleep(2.5)

    window = await desktop.locator(NOTEPAD_WINDOW).timeout(15_000).first()

    # The editable surface is a Document on Windows 11's Notepad and an Edit on the
    # classic one. This is the element whose body we screenshot.
    editor_role = "role:Document" if platform.release() == "11" else "role:Edit"
    document = await window.locator(editor_role).timeout(10_000).first()
    print(f"Found editor body: {document.name()!r}")
    document.click()
    document.type_text(SAMPLE_TEXT)
    await asyncio.sleep(0.5)

    # Highlight the element on screen with a border so it's obvious live which
    # element we're capturing.
    document.highlight(color=0x00FF00, duration_ms=2000)

    print("Capturing screenshot of the editor body...\n")
    screenshot = document.capture()
    print(
        f"Screenshot dimensions: {screenshot.width}x{screenshot.height}, "
        f"data length: {len(screenshot.image_data)}\n"
    )

    print("Converting screenshot to PIL Image...")
    image = Image.frombytes(
        "RGBA", (screenshot.width, screenshot.height), screenshot.image_data
    )
    # Frame the captured element with a border so the saved image also clearly
    # shows which element was screenshotted (the on-screen highlight isn't part of
    # the element's own capture region).
    image = ImageOps.expand(image, border=BORDER_WIDTH, fill=BORDER_COLOR)
    print("Saving screenshot to screenshot.png\n")
    screenshot_path = "screenshot.png"
    image.save(screenshot_path)

    print("Performing OCR on screenshot...")
    text = await desktop.ocr_screenshot(screenshot)
    ocr_result = text if text.strip() else "(no text recognized)"
    print("OCR result:\n")
    print(ocr_result)

    # Write the OCR result back into the same editor body, below the original
    # text and separated by a divider line so the two are clearly distinct.
    print("\nWriting OCR result back into the editor body...")
    document.click()
    document.press_key("{Ctrl}{End}")  # move the cursor to the end of the text
    document.type_text("\n================\n" + ocr_result)
    await asyncio.sleep(0.5)

    # Open the saved screenshot in the default image viewer so the result is
    # shown as soon as the script finishes.
    print()
    open_image(screenshot_path)


async def main():
    desktop = terminator.Desktop(log_level="error")
    try:
        await capture_editor_body(desktop)
    except Exception as e:
        print("Error:", str(e))


if __name__ == "__main__":
    asyncio.run(main())
