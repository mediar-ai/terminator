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
import glob
import os
import shutil
import subprocess
import sys

import terminator


def find_ffmpeg():
    """
    Return the directory containing ffmpeg, or None if it can't be found.

    We don't rely on ffmpeg being on PATH: an editor/terminal launched before
    ffmpeg was installed keeps a stale PATH, so `ffmpeg` may be missing from the
    process environment even though it's installed system-wide. We check PATH
    first, then fall back to winget's standard install locations. The directory
    is passed to yt-dlp via --ffmpeg-location so trimming works regardless.
    """
    on_path = shutil.which("ffmpeg")
    if on_path:
        return os.path.dirname(on_path)

    local = os.environ.get("LOCALAPPDATA", "")
    candidates = [
        # winget shim directory (symlinks to the real binaries).
        os.path.join(local, "Microsoft", "WinGet", "Links", "ffmpeg.exe"),
        # winget package payload, e.g. Gyan.FFmpeg .../ffmpeg-*/bin/ffmpeg.exe.
        *glob.glob(
            os.path.join(
                local, "Microsoft", "WinGet", "Packages",
                "*FFmpeg*", "ffmpeg-*", "bin", "ffmpeg.exe",
            )
        ),
    ]
    for path in candidates:
        if os.path.isfile(path):
            return os.path.dirname(path)
    return None


def ffmpeg_exe():
    """Return the full path to the ffmpeg executable, or raise RuntimeError."""
    ffmpeg_dir = find_ffmpeg()
    if not ffmpeg_dir:
        raise RuntimeError(
            "ffmpeg not found. Install it (e.g. `winget install Gyan.FFmpeg`)."
        )
    return os.path.join(
        ffmpeg_dir, "ffmpeg.exe" if os.name == "nt" else "ffmpeg"
    )


def start_first_minute_stream(link, duration=60, host="127.0.0.1", port=8090):
    """
    Serve the first `duration` seconds of a YouTube (or other site) video as an
    HTTP network stream, and return (ffmpeg_process, url).

    VLC's "Open Network Stream" box only accepts a network MRL (http://, rtsp://,
    ...), not a local file path, so a downloaded clip won't play there. Instead we
    turn the video into an actual network stream:

      1. yt-dlp resolves the page URL to a direct media URL (see resolve_stream_url).
      2. ffmpeg runs as a one-shot HTTP server (-listen 1) that reads that URL,
         trims to the first `duration` seconds (-t) and remuxes to MPEG-TS on the
         fly (-c copy, no re-encode). It blocks until a client connects, then
         streams the trimmed clip.

    VLC then opens http://host:port from the Network Stream dialog and plays it.

    The caller owns the returned process and must terminate it when playback is
    done (see play_livestream_youtube_video's finally block). Raises RuntimeError
    if ffmpeg cannot be located.
    """
    ffmpeg = ffmpeg_exe()

    # Resolve to a direct network URL; ffmpeg cannot read a YouTube page link.
    direct_url = resolve_stream_url(link)

    url = f"http://{host}:{port}"
    print(f"Starting ffmpeg HTTP stream (first {duration}s) at {url} ...")
    proc = subprocess.Popen(
        [
            ffmpeg,
            "-hide_banner",
            "-loglevel", "warning",
            "-i", direct_url,
            "-t", str(duration),      # stop after the first `duration` seconds
            "-c", "copy",             # remux only, no re-encode (fast, cheap)
            "-f", "mpegts",           # container VLC streams happily over HTTP
            "-listen", "1",           # act as an HTTP server for one client
            url,
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return proc, url


def resolve_stream_url(link):
    """
    Resolve a YouTube (or other site) page URL to a direct, playable stream URL
    using yt-dlp.

    VLC 3.0.x cannot play YouTube page links itself: its bundled youtube.lua
    extractor can no longer decipher YouTube's JavaScript-based stream signatures,
    so VLC just shows a blank screen ("Couldn't extract video URL"). yt-dlp handles
    that signature logic and hands us a direct googlevideo URL that VLC plays fine.

    If resolution fails (e.g. the link is already a direct/network stream that VLC
    handles natively), the original link is returned unchanged.
    """
    try:
        result = subprocess.run(
            [sys.executable, "-m", "yt_dlp", "--no-playlist", "-f", "best[ext=mp4]/best", "-g", link],
            capture_output=True,
            text=True,
            timeout=60,
        )
        direct_url = result.stdout.strip().splitlines()
        if result.returncode == 0 and direct_url:
            print("Resolved direct stream URL via yt-dlp.")
            return direct_url[0]
        print(
            "Warning: yt-dlp could not resolve the link; using it as-is. "
            f"({result.stderr.strip().splitlines()[-1] if result.stderr.strip() else 'no details'})"
        )
    except FileNotFoundError:
        print("Warning: yt-dlp not installed (pip install yt-dlp); using link as-is.")
    except subprocess.TimeoutExpired:
        print("Warning: yt-dlp timed out; using link as-is.")
    return link


async def play_livestream_youtube_video(youtube_link):
    """
    Automate VLC to play a YouTube (or any network) stream automatically.
    This script will:
    1. Open VLC media player
    2. Open the 'Open Network Stream' dialog (Ctrl+N)
    3. Paste a YouTube live link into the ComboBox
    4. Click Play to start streaming
    """
    # Serve the first minute as an HTTP network stream. VLC's Network Stream box
    # needs a network MRL (not a file path), so ffmpeg re-streams the trimmed clip
    # over http:// (see start_first_minute_stream). If ffmpeg/yt-dlp fail this
    # raises RuntimeError, which propagates out and aborts the app.
    ffmpeg_proc, stream_url = start_first_minute_stream(youtube_link)

    desktop = terminator.Desktop(log_level="error")
    try:
        print("Opening VLC media player...")
        vlc_window = desktop.open_application(
            "C:\\Program Files\\VideoLAN\\VLC\\vlc.exe"
        )
        await asyncio.sleep(2)

        # Bring VLC to the foreground first. UIElement.press_key only reaches a
        # window while it is focused, so if a stray VLC instance or a leftover
        # error dialog holds focus, Ctrl+N silently goes nowhere. Activating the
        # window makes the shortcut reliable.
        try:
            vlc_window.activate_window()
        except Exception:
            pass
        await asyncio.sleep(0.5)

        print("Opening 'Open Network Stream' dialog (Ctrl+N)...")
        vlc_window.press_key("{Ctrl}n")
        # Alternative (equivalent) via the menu, if you prefer:
        #   media_menu = await vlc_window.locator("Name:Media").first()
        #   media_menu.click()
        #   item = await desktop.locator("Name:Open Network Stream").first()
        #   item.click()
        await asyncio.sleep(1.5)

        open_media_win = desktop.locator("window:Open Media")
        print("Locating and focusing Network Protocol ComboBox...")
        combo = await open_media_win.locator("Name:Network Protocol Down").first()
        combo.click()
        await asyncio.sleep(0.5)
        print(f"Pasting stream link: {stream_url}")
        edit_box = (
            await open_media_win.locator("Name:Network Protocol Down")
            .locator("role:Edit")
            .first()
        )
        edit_box.click()
        edit_box.press_key("{Ctrl}a")
        edit_box.press_key("{Delete}")
        edit_box.type_text(stream_url)
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
    finally:
        # Tear down the ffmpeg HTTP server. With -listen 1 it usually exits once
        # VLC finishes reading the stream, but terminate it explicitly so we never
        # leave the port bound if playback was cut short.
        if ffmpeg_proc.poll() is None:
            ffmpeg_proc.terminate()
            try:
                ffmpeg_proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                ffmpeg_proc.kill()


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
