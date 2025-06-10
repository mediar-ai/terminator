#!/usr/bin/env python3
"""
Cursor Night Shift Agent - Automated Prompt Sender

This example demonstrates how to use Terminator to automate sending prompts to Cursor IDE
at regular intervals. Perfect for running tasks while you're away from the keyboard!

Features:
- Automatically finds and focuses Cursor window
- Sends prompts from a configurable list
- Customizable intervals between prompts
- Graceful error handling and recovery
- Supports both chat and command modes
- Highlights UI elements for visual feedback

Usage:
    python cursor_night_shift_agent.py

Configuration:
    Modify the PROMPTS list and INTERVAL_SECONDS to customize behavior.
"""

import asyncio
import terminator
import time
from typing import List, Dict, Any
import logging

# Configure logging for better debugging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

class CursorNightShiftAgent:
    """
    Automated agent for sending prompts to Cursor IDE at intervals.
    """
    
    def __init__(self, prompts: List[str], interval_seconds: int = 300, max_retries: int = 3):
        """
        Initialize the night shift agent.
        
        Args:
            prompts: List of prompts to send to Cursor
            interval_seconds: Time to wait between prompts (default: 5 minutes)
            max_retries: Maximum number of retries for each operation
        """
        self.prompts = prompts
        self.interval_seconds = interval_seconds
        self.max_retries = max_retries
        self.desktop = terminator.Desktop(log_level="error")
        self.cursor_app = None
        self.current_prompt_index = 0
        
    async def find_cursor_window(self) -> bool:
        """
        Find and focus the Cursor application window.
        
        Returns:
            bool: True if Cursor window found and focused, False otherwise
        """
        try:
            logger.info("Looking for Cursor application...")
            
            # Try different ways to find Cursor
            cursor_selectors = [
                "name:Cursor",
                "name:cursor",
                "window:Cursor",
                "window:cursor"
            ]
            
            for selector in cursor_selectors:
                try:
                    cursor_window = await self.desktop.locator(selector).first()
                    if cursor_window:
                        logger.info(f"Found Cursor window using selector: {selector}")
                        cursor_window.highlight(color=0x00FF00, duration_ms=2000)  # Green highlight
                        await cursor_window.focus()
                        self.cursor_app = cursor_window
                        await asyncio.sleep(1)  # Allow window to focus
                        return True
                except Exception as e:
                    logger.debug(f"Selector {selector} failed: {e}")
                    continue
            
            # If window not found, try to open Cursor
            logger.info("Cursor window not found, attempting to launch...")
            try:
                self.cursor_app = self.desktop.open_application("cursor.exe")
                await asyncio.sleep(5)  # Allow app to fully load
                logger.info("Cursor launched successfully")
                return True
            except Exception as e:
                logger.error(f"Failed to launch Cursor: {e}")
                return False
                
        except Exception as e:
            logger.error(f"Error finding Cursor window: {e}")
            return False
    
    async def find_chat_input(self):
        """
        Find the chat input area in Cursor.
        
        Returns:
            The chat input element if found, None otherwise
        """
        try:
            # Common selectors for chat input in Cursor
            chat_selectors = [
                "role:textbox",
                "role:EditableText", 
                "role:Edit",
                "name:*chat*",
                "name:*input*",
                "name:*message*",
                "placeholder:*message*",
                "placeholder:*chat*"
            ]
            
            for selector in chat_selectors:
                try:
                    chat_input = await self.cursor_app.locator(selector).first()
                    if chat_input and await chat_input.is_visible():
                        logger.info(f"Found chat input using selector: {selector}")
                        chat_input.highlight(color=0x0000FF, duration_ms=1500)  # Blue highlight
                        return chat_input
                except Exception as e:
                    logger.debug(f"Chat selector {selector} failed: {e}")
                    continue
            
            logger.warning("Could not find chat input, will try keyboard shortcuts")
            return None
            
        except Exception as e:
            logger.error(f"Error finding chat input: {e}")
            return None
    
    async def open_chat_with_shortcut(self) -> bool:
        """
        Try to open chat using keyboard shortcuts.
        
        Returns:
            bool: True if successful, False otherwise
        """
        try:
            logger.info("Attempting to open chat with keyboard shortcuts...")
            
            # Common shortcuts to open chat in Cursor
            shortcuts = [
                "{Ctrl}l",      # Ctrl+L (common for chat)
                "{Ctrl}k",      # Ctrl+K (command palette)
                "{Ctrl}{Shift}p",  # Command palette
                "{F1}",         # Help/Command palette
            ]
            
            for shortcut in shortcuts:
                logger.info(f"Trying shortcut: {shortcut}")
                await self.cursor_app.press_key(shortcut)
                await asyncio.sleep(2)
                
                # Check if chat input appeared
                chat_input = await self.find_chat_input()
                if chat_input:
                    logger.info(f"Chat opened successfully with shortcut: {shortcut}")
                    return True
            
            logger.warning("Could not open chat with shortcuts")
            return False
            
        except Exception as e:
            logger.error(f"Error opening chat with shortcuts: {e}")
            return False
    
    async def send_prompt(self, prompt: str) -> bool:
        """
        Send a prompt to Cursor.
        
        Args:
            prompt: The prompt text to send
            
        Returns:
            bool: True if successful, False otherwise
        """
        try:
            logger.info(f"Sending prompt: {prompt[:50]}...")
            
            # Find chat input
            chat_input = await self.find_chat_input()
            
            # If not found, try to open chat
            if not chat_input:
                if not await self.open_chat_with_shortcut():
                    logger.error("Could not find or open chat input")
                    return False
                chat_input = await self.find_chat_input()
            
            if chat_input:
                # Clear existing text and type the prompt
                await chat_input.focus()
                await asyncio.sleep(0.5)
                
                # Clear any existing text
                await chat_input.press_key("{Ctrl}a")
                await asyncio.sleep(0.2)
                
                # Type the prompt
                await chat_input.type_text(prompt)
                await asyncio.sleep(1)
                
                # Send the prompt (try different methods)
                send_methods = [
                    "{Enter}",
                    "{Ctrl}{Enter}",
                    "{Shift}{Enter}"
                ]
                
                for method in send_methods:
                    try:
                        logger.info(f"Attempting to send with: {method}")
                        await chat_input.press_key(method)
                        await asyncio.sleep(2)
                        
                        # Basic success check - if we can still find input, assume it worked
                        test_input = await self.find_chat_input()
                        if test_input:
                            logger.info("Prompt sent successfully!")
                            return True
                    except Exception as e:
                        logger.debug(f"Send method {method} failed: {e}")
                        continue
                
                logger.warning("All send methods failed")
                return False
            else:
                # Fallback: try typing directly to focused window
                logger.info("Fallback: typing directly to focused window")
                await self.cursor_app.focus()
                await asyncio.sleep(1)
                await self.cursor_app.type_text(prompt)
                await asyncio.sleep(1)
                await self.cursor_app.press_key("{Enter}")
                logger.info("Fallback method completed")
                return True
                
        except Exception as e:
            logger.error(f"Error sending prompt: {e}")
            return False
    
    async def run_single_cycle(self) -> bool:
        """
        Run a single cycle of the night shift agent.
        
        Returns:
            bool: True if successful, False otherwise
        """
        try:
            # Get the current prompt
            if self.current_prompt_index >= len(self.prompts):
                self.current_prompt_index = 0  # Loop back to start
            
            prompt = self.prompts[self.current_prompt_index]
            
            logger.info(f"=== Cycle {self.current_prompt_index + 1}/{len(self.prompts)} ===")
            
            # Ensure Cursor is focused
            if not await self.find_cursor_window():
                logger.error("Could not find or focus Cursor window")
                return False
            
            # Send the prompt
            success = await self.send_prompt(prompt)
            
            if success:
                logger.info(f"Successfully sent prompt {self.current_prompt_index + 1}")
                self.current_prompt_index += 1
            else:
                logger.warning(f"Failed to send prompt {self.current_prompt_index + 1}")
            
            return success
            
        except Exception as e:
            logger.error(f"Error in single cycle: {e}")
            return False
    
    async def run(self, max_cycles: int = None) -> None:
        """
        Run the night shift agent continuously.
        
        Args:
            max_cycles: Maximum number of cycles to run (None for infinite)
        """
        logger.info("🌙 Starting Cursor Night Shift Agent...")
        logger.info(f"📝 {len(self.prompts)} prompts configured")
        logger.info(f"⏰ {self.interval_seconds} seconds between prompts")
        logger.info("🔄 Press Ctrl+C to stop")
        
        cycle_count = 0
        consecutive_failures = 0
        
        try:
            while max_cycles is None or cycle_count < max_cycles:
                cycle_count += 1
                
                # Run a cycle
                success = await self.run_single_cycle()
                
                if success:
                    consecutive_failures = 0
                    logger.info(f"✅ Cycle {cycle_count} completed successfully")
                else:
                    consecutive_failures += 1
                    logger.warning(f"❌ Cycle {cycle_count} failed (consecutive failures: {consecutive_failures})")
                    
                    # If too many consecutive failures, longer wait
                    if consecutive_failures >= 3:
                        logger.warning("Multiple failures detected, waiting longer before retry...")
                        await asyncio.sleep(self.interval_seconds * 2)
                        consecutive_failures = 0  # Reset after long wait
                        continue
                
                # Wait for the next cycle
                if max_cycles is None or cycle_count < max_cycles:
                    logger.info(f"😴 Sleeping for {self.interval_seconds} seconds until next prompt...")
                    await asyncio.sleep(self.interval_seconds)
                    
        except KeyboardInterrupt:
            logger.info("🛑 Night shift agent stopped by user")
        except Exception as e:
            logger.error(f"💥 Unexpected error: {e}")
        finally:
            logger.info("🌅 Night shift agent finished")


# Configuration
PROMPTS = [
    "Review the codebase and suggest any potential improvements to error handling",
    "Check for any unused imports or variables in the current project",
    "Look for opportunities to add helpful comments or documentation",
    "Analyze the code for potential performance optimizations",
    "Check if there are any security considerations that should be addressed",
    "Suggest any refactoring opportunities to improve code maintainability",
    "Review the project structure and suggest any organizational improvements",
    "Check for consistent coding style and naming conventions",
    "Look for any TODO or FIXME comments that could be addressed",
    "Suggest any additional tests that might be valuable"
]

# Time between prompts (in seconds)
INTERVAL_SECONDS = 300  # 5 minutes

async def main():
    """
    Main function to run the night shift agent.
    """
    try:
        # Create and run the agent
        agent = CursorNightShiftAgent(
            prompts=PROMPTS,
            interval_seconds=INTERVAL_SECONDS,
            max_retries=3
        )
        
        # Run indefinitely (or set max_cycles for testing)
        await agent.run()
        
    except Exception as e:
        logger.error(f"Failed to start night shift agent: {e}")

if __name__ == "__main__":
    print("""
    🌙 Cursor Night Shift Agent 
    ===========================
    
    This agent will automatically send prompts to Cursor at regular intervals.
    Make sure Cursor is running before starting!
    
    Default configuration:
    - 10 different code review prompts
    - 5 minute intervals between prompts
    - Automatic error recovery
    
    Press Ctrl+C to stop at any time.
    """)
    
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n👋 Goodbye!")