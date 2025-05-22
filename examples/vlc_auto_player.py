"""
VLC Auto Player Example

USAGE:
------
To play a YouTube/live stream:
    python vlc_auto_player.py --youtube-link "https://www.youtube.com/watch?v=YOUR_VIDEO_ID"

To play a local video file:
    python vlc_auto_player.py --file "my_video.mp4"

- Use --youtube-link to play a YouTube or network stream.
- Use --file to play a local video file.
- If both are provided, YouTube takes priority.

"""
import logging
import sys
import os
import time
import random

# Add the python-sdk directory to the path to find the terminator_sdk module
SDK_PATH = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'python-sdk'))
if SDK_PATH not in sys.path:
    sys.path.insert(0, SDK_PATH)

# Now we can import the SDK
from desktop_use import DesktopUseClient, ApiError, ConnectionError, sleep

logging.basicConfig(level=logging.INFO,
                    format='%(levelname)s: %(message)s')

def play_livestream_youtube_video(youtube_link):
    """
    Automate VLC to play a YouTube (or any network) stream automatically.
    This script will:
    1. Open VLC media player
    2. Open the 'Open Network Stream' dialog (Ctrl+N)
    3. Paste a YouTube live link into the ComboBox
    4. Click Play to start streaming
    """
    client = DesktopUseClient()
    try:
        print("Opening VLC media player...")
        client.open_application("vlc.exe")
        time.sleep(2)

        vlc_window = client.locator('window:VLC media player')
        print("Opening 'Open Network Stream' dialog...")
        vlc_window.press_key('{Ctrl}n')
        time.sleep(1.5)

        open_media_win = client.locator('window:Open Media')
        print("Locating and focusing Network Protocol ComboBox...")
        combo = open_media_win.locator('Name:Network Protocol Down')
        combo.click()
        time.sleep(0.5)
        print(f"Pasting YouTube link: {youtube_link}")
        combo.locator('role:Edit').click()
        combo.locator('role:Edit').press_key('{Ctrl}a')
        combo.locator('role:Edit').press_key('{Delete}')
        combo.locator('role:Edit').type_text(youtube_link)
        time.sleep(0.5)

        print("Clicking Play button...")
        play_button = open_media_win.locator('Name:Play Alt+P')
        play_button.click()
        time.sleep(5)
        print("YouTube stream should now be playing in VLC!")
    except ApiError as e:
        print(f"API Error: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")
        print(f"Error details: {str(e)}")

def play_local_video(video_filename='my_video.mp4'):
    """
    Automate VLC to play local videos automatically.
    This script will:
    1. Open VLC media player
    2. Open the file dialog
    3. Search for a specific video by name
    4. Play the video
    5. Demonstrate play/pause functionality
    """
    client = DesktopUseClient()
    try:
        print("Opening VLC media player...")
        client.open_application("vlc.exe")
        time.sleep(2)

        vlc_window = client.locator('window:VLC media player')
        print("Opening Media menu...")
        vlc_window.press_key('{Alt}')
        vlc_window.press_key('m')
        print("Selecting Open File...")
        try:
            client.locator('Name:Open File...').click()
        except Exception:
            vlc_window.press_key('{Ctrl}o')
        time.sleep(1)

        print("Searching for specific video...")
        file_dialog = client.locator('Window:Select one or more files to open')
        time.sleep(1)
        file_name_edit_box = file_dialog.locator('role:ComboBox').locator('role:Edit')
        file_name_edit_box.type_text(video_filename)

        window_elements = client.locator('Name:Select one or more files to open').explore()
        for child in window_elements.children:
            if child.get('role') == 'Button' and child.get('suggested_selector') and child.get('name') == 'Open':
                open_button = file_dialog.locator(child['suggested_selector'])
                open_button.click()
                print("Open button clicked!")
                break
        time.sleep(1)
        print("Video playback started!")
        time.sleep(1)
        print("Waiting 2 seconds before pausing...")
        time.sleep(2)
        print("Pausing video...")
        vlc_window.press_key(' ')
        print("Video paused. Waiting 2 seconds...")
        time.sleep(2)
        print("Resuming video...")
        vlc_window.press_key(' ')
        print("Video resumed!")
    except ApiError as e:
        print(f"API Error: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")
        print(f"Error details: {str(e)}")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Automate VLC to play videos.")
    parser.add_argument('--file', type=str, help='Local video file to play')
    parser.add_argument('--youtube-link', type=str, help='YouTube link to play')
    args = parser.parse_args()

    if args.youtube_link:
        play_livestream_youtube_video(args.youtube_link)
    elif args.file:
        play_local_video(args.file)
    else:
        print("Please provide either --youtube-link or --file.\n")
        parser.print_help()
