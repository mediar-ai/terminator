import os
import sys
import time
import logging
from desktop_use import DesktopUseClient, ApiError, ConnectionError

# Add the python-sdk directory to the path to find the desktop_use module
SDK_PATH = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'python-sdk'))
if SDK_PATH not in sys.path:
    sys.path.insert(0, SDK_PATH)

# Configure logging
logging.basicConfig(level=logging.INFO,
                    format='%(levelname)s: %(message)s')

def run_example():
    """Run the monitor capture example."""
    print("Starting monitor capture example...")

    try:
        # Initialize the desktop client
        client = DesktopUseClient()
        print("Desktop client initialized.")

        # Get the active monitor name
        try:
            active_monitor_name = client.get_active_monitor_name()
            print(f"Active monitor name: {active_monitor_name}")
        except Exception as e:
            print(f"Could not get active monitor name: {e}")
            print("Using a default monitor name instead.")
            # Use a default monitor name - typically "\\\\.\\DISPLAY1" on Windows
            active_monitor_name = "\\\\.\\DISPLAY1"
            print(f"Using default monitor name: {active_monitor_name}")

        # Capture the monitor using the name
        screenshot = client.capture_monitor_by_name(active_monitor_name)
        print("Captured screenshot of monitor:")
        print(f"  Width: {screenshot.width}px")
        print(f"  Height: {screenshot.height}px")
        print(f"  Image data size: {len(screenshot.image_base64)} bytes (base64 encoded)")

        # Wait a moment
        time.sleep(1)

        # Now try to capture the same monitor again
        print("Capturing the same monitor again...")
        screenshot2 = client.capture_monitor_by_name(active_monitor_name)
        print("Second capture successful!")
        print(f"  Width: {screenshot2.width}px")
        print(f"  Height: {screenshot2.height}px")

        print("Monitor capture example finished.")

    except ConnectionError as e:
        print(f"\n{e}", file=sys.stderr)
        print("Please ensure the Terminator server (`cargo run --example server`) is running.", file=sys.stderr)
        sys.exit(1)
    except ApiError as e:
        print(f"\nAPI Error occurred: {e}", file=sys.stderr)
        sys.exit(1)
    except ImportError as e:
        print(f"\nImport Error: {e}", file=sys.stderr)
        print(f"Ensure the SDK path is correct ({SDK_PATH}) and dependencies are installed.", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"\nAn unexpected error occurred: {e}", file=sys.stderr)
        logging.exception("Unexpected error details:") # Log stack trace for unexpected errors
        sys.exit(1)

    print("\n--- Example Finished ---")

if __name__ == "__main__":
    run_example()