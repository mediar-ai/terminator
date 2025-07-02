#!/usr/bin/env python3
"""
MCP Progress Notifications Demo - Shows real-time progress from execute_sequence
Demonstrates how to receive and display progress notifications

pip install mcp python-dotenv colorama
"""

import asyncio
import json
import os
import sys
import uuid
from typing import Optional, Dict, Any
from contextlib import AsyncExitStack
from colorama import init, Fore, Style, Back

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client
from dotenv import load_dotenv

# Initialize colorama for cross-platform colored output
init(autoreset=True)

load_dotenv()  # Load environment variables from .env


class ProgressTrackingClient:
    def __init__(self):
        # Initialize session and client objects
        self.session: Optional[ClientSession] = None
        self.exit_stack = AsyncExitStack()
        self.progress_updates = []
    
    async def connect_to_server(self, server_command: str = "target/release/terminator-mcp-agent"):
        """Connect to the terminator MCP server"""
        try:
            print(f"{Fore.CYAN}🔌 Connecting to {server_command}...{Style.RESET_ALL}")
            
            # Set up the server parameters
            server_params = StdioServerParameters(
                command=server_command,
                args=[],
                env=None
            )
            
            # Create the client transport and connect
            transport = await self.exit_stack.enter_async_context(
                stdio_client(server_params)
            )
            
            # Create the session
            self.session = await self.exit_stack.enter_async_context(
                ClientSession(transport[0], transport[1])
            )
            
            # Initialize the connection
            await self.session.initialize()
            
            # Set up progress notification handler
            self.session.set_notification_handler(
                "notifications/progress",
                self._handle_progress_notification
            )
            
            print(f"{Fore.GREEN}✅ Connected successfully!{Style.RESET_ALL}")
            print(f"{Fore.GREEN}✅ Progress notification handler registered{Style.RESET_ALL}")
            
        except Exception as e:
            print(f"{Fore.RED}❌ Failed to connect: {e}{Style.RESET_ALL}")
            raise
    
    async def _handle_progress_notification(self, params: Dict[str, Any]):
        """Handle incoming progress notifications"""
        self.progress_updates.append(params)
        
        # Extract progress info
        progress = params.get('progress', 0)
        total = params.get('total', 0)
        message = params.get('message', '')
        
        # Calculate percentage
        percentage = (progress / total * 100) if total > 0 else 0
        
        # Create progress bar
        bar_length = 40
        filled_length = int(bar_length * progress // total) if total > 0 else 0
        bar = '█' * filled_length + '░' * (bar_length - filled_length)
        
        # Display progress
        print(f"\r{Fore.YELLOW}Progress: [{bar}] {percentage:.1f}% - {message}{Style.RESET_ALL}", end='', flush=True)
        
        # New line when complete
        if progress == total and total > 0:
            print()  # New line after completion
    
    async def execute_sequence_with_progress(self):
        """Execute a sequence with progress tracking"""
        if not self.session:
            raise RuntimeError("Not connected to MCP server")
        
        # Generate a unique progress token
        progress_token = f"progress-{uuid.uuid4()}"
        
        # Example sequence: Multiple steps to show progress
        sequence = [
            {
                "tool_name": "get_applications",
                "arguments": {
                    "include_tree": False
                }
            },
            {
                "tool_name": "delay",
                "arguments": {
                    "delay_ms": 500
                }
            },
            {
                "tool_name": "capture_screen",
                "arguments": {}
            },
            {
                "tool_name": "delay",
                "arguments": {
                    "delay_ms": 500
                }
            },
            {
                "tool_name": "get_clipboard",
                "arguments": {}
            },
            {
                "tool_name": "delay",
                "arguments": {
                    "delay_ms": 500
                }
            },
            {
                "tool_name": "get_focused_window_tree",
                "arguments": {}
            }
        ]
        
        print(f"\n{Fore.CYAN}🚀 Executing sequence with {len(sequence)} steps{Style.RESET_ALL}")
        print(f"{Fore.CYAN}📊 Progress tracking enabled with token: {progress_token}{Style.RESET_ALL}\n")
        
        # Convert sequence to JSON string
        tools_json = json.dumps(sequence)
        
        # Clear progress updates
        self.progress_updates = []
        
        try:
            # Call execute_sequence with progress token in meta
            result = await self.session.call_tool(
                "execute_sequence",
                arguments={
                    "tools_json": tools_json,
                    "stop_on_error": True,
                    "include_detailed_results": True
                },
                meta={"progressToken": progress_token}  # Include progress token
            )
            
            # Give a moment for final progress updates
            await asyncio.sleep(0.5)
            
            # Display final results
            print(f"\n{Fore.GREEN}{'='*60}{Style.RESET_ALL}")
            print(f"{Fore.GREEN}✅ SEQUENCE COMPLETED{Style.RESET_ALL}")
            print(f"{Fore.GREEN}{'='*60}{Style.RESET_ALL}")
            
            # Parse and display results
            if result.content and len(result.content) > 0:
                content = result.content[0]
                if hasattr(content, 'text'):
                    result_data = json.loads(content.text)
                else:
                    result_data = content
                
                # Display summary
                if 'execution_summary' in result_data:
                    summary = result_data['execution_summary']
                    print(f"\n{Fore.BLUE}📊 Execution Summary:{Style.RESET_ALL}")
                    print(f"   Total steps: {summary.get('total_steps', 0)}")
                    print(f"   Successful: {summary.get('successful_steps', 0)}")
                    print(f"   Failed: {summary.get('failed_steps', 0)}")
                    print(f"   Duration: {summary.get('total_duration_ms', 0)}ms")
                
                # Display progress history
                print(f"\n{Fore.BLUE}📜 Progress History:{Style.RESET_ALL}")
                for i, update in enumerate(self.progress_updates):
                    print(f"   {i+1}. [{update.get('progress', 0)}/{update.get('total', 0)}] {update.get('message', '')}")
                
        except Exception as e:
            print(f"\n{Fore.RED}❌ Error executing sequence: {e}{Style.RESET_ALL}")
            raise
    
    async def demo_interactive_sequence(self):
        """Demo with an interactive sequence that shows real-time progress"""
        if not self.session:
            raise RuntimeError("Not connected to MCP server")
        
        print(f"\n{Fore.YELLOW}🎮 Interactive Progress Demo{Style.RESET_ALL}")
        print(f"{Fore.YELLOW}This demo will open Notepad and perform several actions{Style.RESET_ALL}")
        print(f"{Fore.YELLOW}Watch the progress bar update in real-time!{Style.RESET_ALL}\n")
        
        input("Press Enter to start the demo...")
        
        # Generate progress token
        progress_token = f"demo-{uuid.uuid4()}"
        
        # Interactive sequence
        sequence = [
            {
                "tool_name": "open_application",
                "arguments": {
                    "app_name": "notepad"
                },
                "delay_ms": 1000
            },
            {
                "tool_name": "delay",
                "arguments": {
                    "delay_ms": 2000
                }
            },
            {
                "tool_name": "type_into_element",
                "arguments": {
                    "selector": "Document|Edit",
                    "text_to_type": "Progress notifications in MCP!\n\n",
                    "clear_before_typing": False
                },
                "delay_ms": 500
            },
            {
                "tool_name": "type_into_element",
                "arguments": {
                    "selector": "Document|Edit",
                    "text_to_type": "Step 1: Opening application ✓\n",
                    "clear_before_typing": False
                },
                "delay_ms": 500
            },
            {
                "tool_name": "type_into_element",
                "arguments": {
                    "selector": "Document|Edit",
                    "text_to_type": "Step 2: Typing text ✓\n",
                    "clear_before_typing": False
                },
                "delay_ms": 500
            },
            {
                "tool_name": "type_into_element",
                "arguments": {
                    "selector": "Document|Edit",
                    "text_to_type": "Step 3: Progress tracking complete! 🎉",
                    "clear_before_typing": False
                }
            }
        ]
        
        tools_json = json.dumps(sequence)
        self.progress_updates = []
        
        try:
            # Execute with progress tracking
            result = await self.session.call_tool(
                "execute_sequence",
                arguments={
                    "tools_json": tools_json,
                    "stop_on_error": True,
                    "include_detailed_results": True
                },
                meta={"progressToken": progress_token}
            )
            
            await asyncio.sleep(0.5)
            
            print(f"\n{Fore.GREEN}🎉 Demo completed successfully!{Style.RESET_ALL}")
            
        except Exception as e:
            print(f"\n{Fore.RED}❌ Demo error: {e}{Style.RESET_ALL}")
    
    async def cleanup(self):
        """Clean up resources"""
        try:
            await self.exit_stack.aclose()
        except (asyncio.CancelledError, Exception):
            pass


async def main():
    """Main entry point"""
    client = ProgressTrackingClient()
    
    try:
        # Connect to the MCP server
        await client.connect_to_server()
        
        # Add a small delay to ensure connection is established
        await asyncio.sleep(1)
        
        # Run the simple progress demo
        print(f"\n{Fore.CYAN}{'='*60}{Style.RESET_ALL}")
        print(f"{Fore.CYAN}📊 Demo 1: Simple Progress Tracking{Style.RESET_ALL}")
        print(f"{Fore.CYAN}{'='*60}{Style.RESET_ALL}")
        await client.execute_sequence_with_progress()
        
        # Ask if user wants to run interactive demo
        print(f"\n{Fore.YELLOW}Would you like to run the interactive Notepad demo?{Style.RESET_ALL}")
        response = input("This will open Notepad and type text. Continue? (y/n): ")
        
        if response.lower() == 'y':
            await client.demo_interactive_sequence()
        
    finally:
        # Clean up
        await client.cleanup()


if __name__ == "__main__":
    print(f"{Fore.CYAN}{'='*60}{Style.RESET_ALL}")
    print(f"{Fore.CYAN}🚀 MCP Progress Notifications Demo{Style.RESET_ALL}")
    print(f"{Fore.CYAN}{'='*60}{Style.RESET_ALL}")
    print(f"\nThis demo shows how to use progress notifications with")
    print(f"the execute_sequence tool for real-time progress tracking.\n")
    
    # Check if terminator-mcp-agent is available
    if not os.path.exists("target/release/terminator-mcp-agent"):
        print(f"{Fore.YELLOW}⚠️  Warning: terminator-mcp-agent not found in target/release/{Style.RESET_ALL}")
        print(f"Please build it first with: cargo build --release --bin terminator-mcp-agent\n")
    
    # Run the async main function
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print(f"\n{Fore.YELLOW}👋 Demo interrupted by user{Style.RESET_ALL}")