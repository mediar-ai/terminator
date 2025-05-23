#!/usr/bin/env python3
"""
YouTube Opener

This script simply opens YouTube in your default browser.
"""

import logging
from time import sleep
from desktop_use import DesktopUseClient

# Set up basic logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)

def open_youtube():
    """Open YouTube in the default browser."""
    try:
        # Initialize the client
        client = DesktopUseClient()
        
        # Open YouTube in the default browser
        youtube_url = "https://www.youtube.com"
        logging.info(f"Opening YouTube: {youtube_url}")
        client.open_url(youtube_url)

        sleep(10)

        app = client.locator('window:YouTube').locator('Document:YouTube')
        masterhead = app.locator('AutomationId:masthead')
        guide_button = masterhead.locator('AutomationId:guide-button')
        guide_button.click()     
        logging.info("YouTube is now open")
            
    except KeyboardInterrupt:
        logging.info("\nExiting...")
    except Exception as e:
        logging.error(f"An error occurred: {str(e)}", exc_info=True)

if __name__ == "__main__":
    open_youtube()
