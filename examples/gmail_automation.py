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

def open_gmail_compose(client: DesktopUseClient, recipient: str, subject: str, body: str):
    """
    Opens Gmail compose window and fills in email details.
    
    Args:
        client: DesktopUseClient instance
        recipient: Email address to send to
        subject: Subject of the email
        body: Body content of the email
    """
    try:
        # Open Gmail in default browser
        gmail_url = "https://mail.google.com/mail/u/0/#inbox?compose=new"
        logging.info("Opening Gmail compose window...")
        client.open_url(gmail_url)
        
        # Wait for the page to load
        sleep(7)  # Adjust this based on your internet speed
        
        # Get the Gmail window
        gmail_window = client.locator('window:Gmail')
        
        # Find and fill recipient field
        recipient_field = gmail_window.locator('Name:To')
        recipient_field.type_text(recipient)
        sleep(1)
        
        # Find and fill subject field
        subject_field = gmail_window.locator('Name:Subject')
        subject_field.type_text(subject)
        sleep(1)
        
        # Find and fill body field
        body_field = gmail_window.locator('Name:Message Body')  # Email body is typically a textbox
        body_field.type_text(body)
        sleep(1)
        
        logging.info("Email composed successfully!")

        # Find and click the Send button
        send_button = gmail_window.locator('Name:Send ‪(Ctrl-Enter)')
        send_button.click()
        sleep(1)

        logging.info("Email sent successfully!")

        sent_button = gmail_window.locator('name:Labels').locator('name:Sent')
        print(sent_button.explore())
        sent_button.click()
        sleep(2)

        grid_item = gmail_window.locator('role:DataGrid').locator('role:DataItem')
        grid_item.click()

        logging.info("Email opened successfully!")
        
    except ApiError as e:
        logging.error(f"API Error: {e}")
        raise
    except Exception as e:
        logging.error(f"An unexpected error occurred: {e}")
        raise

def main():
    client = DesktopUseClient()
    
    # Email details
    recipient = "louis.beaumont@gmail.com"  # Replace with actual recipient
    subject = "Le Terminator est immortel."
    body = "This is a test email sent using Terminator automation."
    
    try:
        open_gmail_compose(client, recipient, subject, body)
    except Exception as e:
        logging.error(f"Failed to compose email: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
