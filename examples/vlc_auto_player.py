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

import asyncio
import terminator


async def play_livestream_youtube_video(youtube_link):
    """
    Automate VLC to play a YouTube (or any network) stream automatically.
    This script will:
    1. Open VLC media player
    2. Open the 'Open Network Stream' dialog (Ctrl+N)
    3. Paste a YouTube live link into the ComboBox
    4. Click Play to start streaming
    """
    desktop = terminator.Desktop(log_level="error")
    try:
        print("Opening VLC media player...")
        vlc_window = desktop.open_application(
            "C:\\Program Files\\VideoLAN\\VLC\\vlc.exe"
        )
        await asyncio.sleep(2)

        print("Opening 'Open Network Stream' dialog...")
        vlc_window.press_key("{Ctrl}n")
        await asyncio.sleep(1.5)

        open_media_win = desktop.locator("window:Open Media")
        print("Locating and focusing Network Protocol ComboBox...")
        combo = await open_media_win.locator("Name:Network Protocol Down").first()
        combo.click()
        await asyncio.sleep(0.5)
        print(f"Pasting YouTube link: {youtube_link}")
        edit_box = (
            await open_media_win.locator("Name:Network Protocol Down")
            .locator("role:Edit")
            .first()
        )
        edit_box.click()
        edit_box.press_key("{Ctrl}a")
        edit_box.press_key("{Delete}")
        edit_box.type_text(youtube_link)
        await asyncio.sleep(0.5)

        print("Clicking Play button...")
        play_button = await open_media_win.locator("Name:Play Alt+P").first()
        play_button.click()
        await asyncio.sleep(5)
        print("YouTube stream should now be playing in VLC!")
    except terminator.PlatformError as e:
        print(f"Platform Error: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")
        print(f"Error details: {str(e)}")


async def play_local_video(video_filename="my_video.mp4"):
    """
    Automate VLC to play local videos automatically.
    This script will:
    1. Open VLC media player
    2. Open the file dialog
    3. Search for a specific video by name
    4. Play the video
    5. Demonstrate play/pause functionality
    """
    desktop = terminator.Desktop(log_level="error")
    try:
        print("Opening VLC media player...")
        vlc_window = desktop.open_application("vlc.exe")
        await asyncio.sleep(2)

        print("Opening Media menu...")
        vlc_window.press_key("{Alt}")
        vlc_window.press_key("m")
        print("Selecting Open File...")
        try:
            open_file_btn = await desktop.locator("Name:Open File...").first()
            open_file_btn.click()
        except Exception:
            vlc_window.press_key("{Ctrl}o")
        await asyncio.sleep(1)

        print("Searching for specific video...")
        file_dialog = await desktop.locator(
            "Window:Select one or more files to open"
        ).first()
        await asyncio.sleep(1)
        file_name_edit_box = (
            await file_dialog.locator("role:ComboBox").locator("role:Edit").first()
        )
        file_name_edit_box.type_text(video_filename)

        for child in file_dialog.children():
            if child.role() == "Button" and child.name() == "Open":
                child.click()
                print("Open button clicked!")
                break
        await asyncio.sleep(1)
        print("Video playback started!")
        await asyncio.sleep(1)
        print("Waiting 2 seconds before pausing...")
        await asyncio.sleep(2)
        print("Pausing video...")
        vlc_window.press_key(" ")
        print("Video paused. Waiting 2 seconds...")
        await asyncio.sleep(2)
        print("Resuming video...")
        vlc_window.press_key(" ")
        print("Video resumed!")
    except terminator.PlatformError as e:
        print(f"Platform Error: {e}")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")
        print(f"Error details: {str(e)}")


async def main():
    import argparse

    parser = argparse.ArgumentParser(description="Automate VLC to play videos.")
    parser.add_argument("--file", type=str, help="Local video file to play")
    parser.add_argument("--youtube-link", type=str, help="YouTube link to play")
    args = parser.parse_args()

    if args.youtube_link:
        await play_livestream_youtube_video(args.youtube_link)
    elif args.file:
        await play_local_video(args.file)
    else:
        print("Please provide either --youtube-link or --file.\n")
        parser.print_help()


if __name__ == "__main__":
    asyncio.run(main())
