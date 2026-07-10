import asyncio
import itertools
import os
import shutil
import subprocess
import time

import terminator
import logging

try:
    from PIL import Image
except ImportError:
    Image = None

# Configure logging
logging.basicConfig(level=logging.INFO, format="%(levelname)s: %(message)s")

# Directory for step-by-step verification screenshots.
SCREENSHOT_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "screenshots")
_step_counter = itertools.count(1)


def save_screenshot(element: "terminator.UIElement", label: str) -> None:
    """
    Capture a screenshot of `element` and save it to SCREENSHOT_DIR.

    Used to verify each automation step ran against the correct window. Never
    raises -- a screenshot failure must not abort the automation.
    """
    if Image is None:
        logging.warning("Pillow not installed; skipping screenshot '%s'.", label)
        return
    try:
        os.makedirs(SCREENSHOT_DIR, exist_ok=True)
        shot = element.capture()
        img = Image.frombytes("RGBA", (shot.width, shot.height), shot.image_data)
        n = next(_step_counter)
        path = os.path.join(SCREENSHOT_DIR, f"{n:02d}_{label}.png")
        img.save(path)
        logging.info("Screenshot saved: %s", path)
    except Exception as e:  # noqa: BLE001 - screenshots are best-effort
        logging.warning("Could not capture screenshot '%s': %s", label, e)

# Selector for the Chrome browser window. Chrome exposes its top-level window
# as a *Pane* whose name is the tab title, e.g.
#   "Inbox (12) - you@gmail.com - Gmail - Google Chrome"
# We match on "Gmail - Google Chrome" (not just "Gmail") so it can't accidentally
# bind to the VS Code window, whose title also contains "gmail" when this file is
# open. `name:contains:` is a case-insensitive substring match.
GMAIL_WINDOW = "role:Pane|name:contains:Gmail - Google Chrome"

# How long to wait for the Gmail compose field to appear after Chrome launches.
# This covers the whole sign-in flow, including typing a password AND entering an
# authenticator-app 2FA token, so keep it generous. The script does not block for
# the full duration -- it proceeds the moment the field is found.
LOGIN_TIMEOUT_MS = 180_000  # 3 minutes

# Timeout for UI elements once Gmail is already loaded (e.g. the compose fields).
COMPOSE_TIMEOUT_MS = 30_000  # 30 seconds

# Dedicated Chrome profile for automation. Launched with its own --user-data-dir so
# it runs as a separate instance alongside your everyday browser (no need to close
# your normal Chrome). It is also launched with `--force-renderer-accessibility`,
# which is REQUIRED: without it Chrome does not expose web-page content to the OS
# accessibility tree, so Terminator cannot see the Gmail fields. Sign into Gmail in
# the window that opens on first run; the profile stays signed in for later runs.
AUTOMATION_PROFILE = os.path.join(
    os.path.expanduser("~"), ".terminator_chrome_profile"
)


def _find_chrome() -> str:
    """Locate the Chrome executable on Windows."""
    candidates = [
        shutil.which("chrome"),
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        os.path.expandvars(r"%LOCALAPPDATA%\Google\Chrome\Application\chrome.exe"),
    ]
    for path in candidates:
        if path and os.path.exists(path):
            return path
    raise FileNotFoundError(
        "Could not find chrome.exe. Set the path manually in _find_chrome()."
    )


def _close_automation_chrome() -> None:
    """
    Close only the Chrome processes that belong to AUTOMATION_PROFILE.

    Repeated runs otherwise accumulate stale Gmail windows (old drafts, opened
    emails), which makes the window selectors ambiguous. We filter by command line
    so this NEVER touches your everyday Chrome -- only the automation profile.
    """
    profile_marker = os.path.basename(AUTOMATION_PROFILE)  # ".terminator_chrome_profile"
    ps = (
        "Get-CimInstance Win32_Process -Filter \"Name='chrome.exe'\" | "
        f"Where-Object {{ $_.CommandLine -like '*{profile_marker}*' }} | "
        "ForEach-Object { Stop-Process -Id $_.ProcessId -Force }"
    )
    subprocess.run(
        ["powershell.exe", "-NoProfile", "-Command", ps],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def launch_gmail_chrome(url: str) -> None:
    """
    Launch Chrome with renderer accessibility forced on and open the given URL.

    Uses a dedicated --user-data-dir (AUTOMATION_PROFILE) so it runs as its own
    instance without disturbing your everyday Chrome. `--force-renderer-accessibility`
    is the key flag: it makes Chrome publish the web page to the accessibility tree
    so Terminator can find the Gmail fields.
    """
    chrome = _find_chrome()
    # Start from a clean slate: close any leftover automation windows first so there
    # is exactly one Gmail window and the selectors stay unambiguous.
    _close_automation_chrome()
    time.sleep(2)
    logging.info("Launching Chrome (automation profile) with accessibility enabled...")
    subprocess.Popen(
        [
            chrome,
            "--force-renderer-accessibility",
            f"--user-data-dir={AUTOMATION_PROFILE}",
            "--no-first-run",
            "--no-default-browser-check",
            "--hide-crash-restore-bubble",  # suppress "Chrome didn't shut down correctly"
            "--new-window",
            # Start maximized so the whole Gmail UI (Compose + Send buttons) is on
            # screen. Terminator can only click elements that are actually rendered;
            # a small or partially off-screen window leaves those controls invisible
            # and the automation fails. See also the maximize_window() call after the
            # window is activated, which enforces this state at runtime.
            "--start-maximized",
            url,
        ]
    )


async def open_gmail_compose(
    desktop: terminator.Desktop, recipient: str, subject: str, body: str
):
    """
    Opens Gmail compose window and fills in email details.

    Args:
        desktop: terminator.Desktop instance
        recipient: Email address to send to
        subject: Subject of the email
        body: Body content of the email
    """
    try:
        # Open the Gmail inbox in a Chrome instance that exposes web accessibility.
        # We do NOT use the "?compose=new" URL trick: on a fresh/first load it can
        # open the compose dialog before the inbox is interactive (or not at all).
        # Instead we wait for the inbox, then click the Compose button ourselves.
        gmail_url = "https://mail.google.com/mail/u/0/#inbox"
        logging.info("Opening Gmail...")
        launch_gmail_chrome(gmail_url)

        # Scope all lookups to the Gmail browser window. We search descendants
        # directly (Chrome nests the page content several Panes deep, so there is
        # no reliable single "Document" node to chain through).
        gmail_window = desktop.locator(GMAIL_WINDOW)

        # Wait for the inbox to finish loading, keyed on the Compose button. This
        # uses a long timeout so it tolerates the full sign-in flow (password AND
        # authenticator 2FA token) plus the first-load of a large mailbox.
        logging.info(
            "Waiting up to %ds for Gmail to load "
            "(complete sign-in and 2FA if prompted)...",
            LOGIN_TIMEOUT_MS // 1000,
        )
        compose_button = (
            await gmail_window.locator("name:Compose")
            .timeout(LOGIN_TIMEOUT_MS)
            .first()
        )

        # Pin to the exact window that owns the Compose button. Your everyday Chrome
        # may also have a "... - Gmail - Google Chrome" window, so GMAIL_WINDOW can
        # match several; only the automation window exposes accessible web content
        # (hence the Compose button). Scoping every later lookup to this element
        # keeps us out of the wrong window. Bring it to the foreground too, because
        # type_text/press_key go to whatever window currently has focus.
        window_el = compose_button.window() or await gmail_window.first()
        window_el.activate_window()
        # Force the window into a known, fully-visible state. Terminator can only
        # click controls that are actually rendered on screen, so if the window is
        # small or partially off-screen the Compose/Send buttons are unreachable and
        # the run fails. Maximizing (in addition to --start-maximized at launch)
        # makes the window state reproducible regardless of how Chrome last closed.
        try:
            window_el.maximize_window()
        except Exception as e:  # noqa: BLE001 - best-effort; continue if unsupported
            logging.warning("Could not maximize the Gmail window: %s", e)
        await asyncio.sleep(1)
        save_screenshot(window_el, "gmail_loaded")

        # Open the compose dialog and wait for the recipient field (a ComboBox
        # named "To recipients").
        logging.info("Opening compose window...")
        compose_button.click()
        recipient_field = (
            await window_el.locator("role:ComboBox|name:To recipients")
            .timeout(COMPOSE_TIMEOUT_MS)
            .first()
        )
        save_screenshot(window_el, "compose_opened")

        recipient_field.highlight(color=0x00FF00, duration_ms=2000)  # Green highlight
        recipient_field.click()  # ensure focus before typing
        recipient_field.type_text(recipient)
        # Commit the recipient and dismiss the autocomplete dropdown, otherwise it
        # can steal the keystrokes intended for the next field.
        recipient_field.press_key("{Escape}")
        await asyncio.sleep(1)
        save_screenshot(window_el, "recipient_filled")

        # Find and fill subject field. We must pin the role to Edit: a bare
        # "name:Subject" also matches inbox email rows whose preview text contains
        # "Subject:", and .first() would grab one of those instead of the compose
        # box -- which is exactly why the subject was landing in the wrong place.
        subject_field = (
            await window_el.locator("role:Edit|name:Subject")
            .timeout(COMPOSE_TIMEOUT_MS)
            .first()
        )
        subject_field.highlight(color=0x0000FF, duration_ms=2000)  # Blue highlight
        subject_field.click()  # focus the subject box before typing
        await asyncio.sleep(0.3)
        subject_field.type_text(subject)
        await asyncio.sleep(0.5)
        # Verify the subject actually landed; warn loudly if not.
        typed = (subject_field.text() or "").strip()
        if subject not in typed:
            logging.warning(
                "Subject may not have been entered (field text=%r). Retrying...",
                typed,
            )
            subject_field.click()
            await asyncio.sleep(0.3)
            subject_field.type_text(subject)
        await asyncio.sleep(1)
        save_screenshot(window_el, "subject_filled")

        # Find and fill body field
        body_field = (
            await window_el.locator("role:Edit|name:Message Body")
            .timeout(COMPOSE_TIMEOUT_MS)
            .first()
        )
        body_field.highlight(color=0xFF00FF, duration_ms=2000)  # Magenta highlight
        body_field.click()  # focus the body before typing
        await asyncio.sleep(0.3)
        body_field.type_text(body)
        await asyncio.sleep(1)
        save_screenshot(window_el, "body_filled")

        logging.info("Email composed successfully!")

        # Find and click the Send button (labelled "Send \u202A(Ctrl-Enter)\u202C")
        send_button = await window_el.locator("name:Ctrl-Enter").first()
        send_button.highlight(color=0xFFFF00, duration_ms=2000)  # Yellow highlight
        send_button.click()
        await asyncio.sleep(2)
        save_screenshot(window_el, "email_sent")

        logging.info("Email sent successfully!")

        # Navigate to Sent folder
        sent_button = await window_el.locator("name:Sent").first()
        sent_button.highlight(color=0x00FFFF, duration_ms=2000)  # Cyan highlight
        sent_button.click()
        await asyncio.sleep(2)
        save_screenshot(window_el, "sent_folder")

        # Open the sent email. Gmail's message list uses DataItem rows (there is no
        # "DataGrid" role), so we locate the row by its body preview text. This is a
        # verification convenience -- the email is already sent -- so a failure here
        # is logged but does not fail the run.
        try:
            snippet = body[:25]
            sent_item = (
                await window_el.locator(f"role:DataItem|name:contains:{snippet}")
                .timeout(COMPOSE_TIMEOUT_MS)
                .first()
            )
            sent_item.highlight(color=0xFFA500, duration_ms=2000)  # Orange highlight
            sent_item.click()
            await asyncio.sleep(1)
            save_screenshot(window_el, "email_opened")
            logging.info("Email opened successfully!")
        except Exception as e:  # noqa: BLE001 - opening the sent email is optional
            logging.warning("Could not open the sent email (non-fatal): %s", e)
            save_screenshot(window_el, "email_open_failed")

    except terminator.PlatformError as e:
        logging.error(f"Platform Error: {e}")
        await _capture_failure(desktop)
        raise
    except Exception as e:
        logging.error(f"An unexpected error occurred: {e}")
        await _capture_failure(desktop)
        raise


async def _capture_failure(desktop: terminator.Desktop) -> None:
    """Best-effort full-screen capture when a step fails, for debugging."""
    if Image is None:
        return
    try:
        monitor = await desktop.get_primary_monitor()
        shot = await desktop.capture_monitor(monitor)
        os.makedirs(SCREENSHOT_DIR, exist_ok=True)
        img = Image.frombytes("RGBA", (shot.width, shot.height), shot.image_data)
        path = os.path.join(SCREENSHOT_DIR, "99_FAILURE.png")
        img.save(path)
        logging.info("Failure screenshot saved: %s", path)
    except Exception as e:  # noqa: BLE001
        logging.warning("Could not capture failure screenshot: %s", e)


async def main():
    desktop = terminator.Desktop(log_level="error")  # Suppress info logs

    # Email details
    recipient = "your_email@gmail.com"  # Replace with actual recipient
    subject = "Le Terminator est immortel."
    body = "This is a test email sent using Terminator automation."

    try:
        await open_gmail_compose(desktop, recipient, subject, body)
    except Exception as e:
        logging.error(f"Failed to compose email: {e}")
        raise


if __name__ == "__main__":
    asyncio.run(main())
