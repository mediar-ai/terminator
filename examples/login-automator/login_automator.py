import logging
import os
import sys
import time

SDK_PATH = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', 'python-sdk'))
if SDK_PATH not in sys.path:
    sys.path.insert(0, SDK_PATH)

from desktop_use import DesktopUseClient, ApiError, ConnectionError

logging.basicConfig(level=logging.INFO, format='%(levelname)s: %(message)s')

USERNAME = os.getenv('DEMO_USERNAME', 'demo@example.com')
PASSWORD = os.getenv('DEMO_PASSWORD', 'password123')


def run_login_automation():
    client = DesktopUseClient()
    login_script = os.path.join(os.path.dirname(__file__), 'dummy_login.py')

    logging.info('Launching dummy login window...')
    client.run_command(windows_command=f'{sys.executable} "{login_script}"',
                       unix_command=f'{sys.executable} "{login_script}"')

    time.sleep(2)  # Allow window to appear

    try:
        window = client.locator('window:Dummy Login')

        logging.info('Filling credentials...')
        window.locator('name:username').type_text(USERNAME)
        window.locator('name:password').type_text(PASSWORD)
        window.locator('name:login_button').click()

        logging.info('Automation complete!')
    except ApiError as e:
        logging.error(f'API error: {e}')
    except ConnectionError:
        logging.error('Could not connect to Terminator server.')
    except Exception as e:
        logging.error(f'Unexpected error: {e}')


if __name__ == '__main__':
    run_login_automation()
