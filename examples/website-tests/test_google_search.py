#!/usr/bin/env python3
"""
Example website test using Terminator SDK
Tests Google search functionality by:
1. Opening Google homepage
2. Searching for a term
3. Verifying search results appear
"""

import asyncio
import terminator
import logging
import os
import sys

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(levelname)s: %(message)s')

async def test_google_search():
    """Test Google search functionality"""
    
    # Get configuration from environment variables
    url = os.getenv('TERMINATOR_URL', 'https://www.google.com')
    timeout = int(os.getenv('TERMINATOR_TIMEOUT', '60'))
    search_term = os.getenv('SEARCH_TERM', 'Terminator SDK automation')
    
    logging.info(f"Starting Google search test with URL: {url}")
    logging.info(f"Search term: {search_term}")
    
    desktop = terminator.Desktop(log_level="info")
    
    try:
        # Step 1: Open Google homepage
        logging.info("Opening Google homepage...")
        desktop.open_url(url)
        await asyncio.sleep(3)  # Wait for page to load
        
        # Step 2: Find Google window and search box
        logging.info("Looking for Google search interface...")
        try:
            # Try to find the window containing Google
            google_window = desktop.locator('window:Google')
            document = google_window.locator('role:Document')
        except Exception:
            # Fallback: use any browser window
            logging.info("Fallback: Using any available browser window")
            google_window = desktop.locator('role:Window')
            document = google_window.locator('role:Document')
        
        # Wait a bit more for page to fully load
        await asyncio.sleep(2)
        
        # Step 3: Find and interact with search box
        logging.info("Finding search input field...")
        
        # Try different ways to find the search box
        search_box = None
        search_attempts = [
            'name:Search',
            'name:q',
            'role:TextBox',
            'role:SearchBox',
            'name:Google Search'
        ]
        
        for attempt in search_attempts:
            try:
                logging.info(f"Trying to find search box with: {attempt}")
                search_box = await document.locator(attempt).first()
                break
            except Exception as e:
                logging.debug(f"Failed with {attempt}: {e}")
                continue
        
        if not search_box:
            raise Exception("Could not find Google search box")
        
        # Step 4: Type search term
        logging.info(f"Typing search term: {search_term}")
        search_box.highlight(color=0x00FF00, duration_ms=1000)  # Green highlight
        await search_box.clear()  # Clear any existing text
        await search_box.type_text(search_term)
        await asyncio.sleep(1)
        
        # Step 5: Submit search (try Enter key first, then search button)
        logging.info("Submitting search...")
        try:
            await search_box.press_key("Return")
            await asyncio.sleep(3)  # Wait for search results
        except Exception:
            # Fallback: try to find and click search button
            logging.info("Trying search button instead...")
            try:
                search_button = await document.locator('name:Google Search').first()
                search_button.highlight(color=0x0000FF, duration_ms=1000)  # Blue highlight
                await search_button.click()
                await asyncio.sleep(3)
            except Exception as e:
                logging.error(f"Could not submit search: {e}")
                raise
        
        # Step 6: Verify search results
        logging.info("Verifying search results...")
        
        # Try to find search results container
        results_found = False
        result_attempts = [
            'role:Main',
            'name:Search Results',
            'role:List'
        ]
        
        for attempt in result_attempts:
            try:
                logging.info(f"Looking for results with: {attempt}")
                results_container = await document.locator(attempt).first()
                if results_container:
                    results_container.highlight(color=0xFFFF00, duration_ms=1000)  # Yellow highlight
                    results_found = True
                    logging.info("✅ Search results found!")
                    break
            except Exception as e:
                logging.debug(f"No results found with {attempt}: {e}")
                continue
        
        if not results_found:
            # Try alternative verification - look for any links on the page
            try:
                links = await document.locator('role:Link').count()
                if links > 5:  # If we have several links, assume search worked
                    logging.info(f"✅ Found {links} links - search appears successful")
                    results_found = True
            except Exception:
                pass
        
        if not results_found:
            raise Exception("No search results found - search may have failed")
        
        # Step 7: Take a screenshot for verification (optional)
        try:
            logging.info("Taking screenshot of results...")
            screenshot_result = await desktop.screenshot("test-outputs/google_search_results.png")
            if screenshot_result:
                logging.info("Screenshot saved successfully")
        except Exception as e:
            logging.warning(f"Could not take screenshot: {e}")
        
        logging.info("🎉 Google search test completed successfully!")
        return True
        
    except Exception as e:
        logging.error(f"Test failed: {e}")
        
        # Try to take a failure screenshot
        try:
            await desktop.screenshot("test-outputs/google_search_failure.png")
            logging.info("Failure screenshot saved")
        except Exception:
            pass
        
        raise

async def main():
    """Main test execution"""
    try:
        success = await test_google_search()
        if success:
            logging.info("All tests passed! ✅")
            sys.exit(0)
        else:
            logging.error("Tests failed! ❌")
            sys.exit(1)
    except Exception as e:
        logging.error(f"Test execution failed: {e}")
        sys.exit(1)

if __name__ == "__main__":
    asyncio.run(main())