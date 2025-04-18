import time
from desktop_use import DesktopUseClient

def main():
    print("Starting monitor capture example...")

    # Initialize the desktop client
    client = DesktopUseClient()
    print("Desktop client initialized.")

    # Get the active monitor name
    active_monitor_name = client.get_active_monitor_name()
    print(f"Active monitor name: {active_monitor_name}")

    # Capture the active monitor using its name
    screenshot = client.capture_monitor_by_name(active_monitor_name)
    print("Captured screenshot of active monitor:")
    print(f"  Width: {screenshot.width}px")
    print(f"  Height: {screenshot.height}px")
    print(f"  Image data size: {len(screenshot.image_base64)} bytes (base64 encoded)")

    # Wait a moment
    time.sleep(1)

    # Now try to capture a specific monitor by name
    # This will use the same name we got from get_active_monitor_name
    print("Capturing the same monitor again...")
    screenshot2 = client.capture_monitor_by_name(active_monitor_name)
    print("Second capture successful!")
    print(f"  Width: {screenshot2.width}px")
    print(f"  Height: {screenshot2.height}px")

    print("Monitor capture example finished.")

if __name__ == "__main__":
    main()