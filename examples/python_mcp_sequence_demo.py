#!/usr/bin/env python3
"""
MCP Execute Sequence Demo - Shows detailed step-by-step execution
Demonstrates how to use the execute_sequence tool and display progress

pip install mcp python-dotenv colorama
"""

import asyncio
import json
import os
import sys
import time
from typing import Optional, List, Dict, Any
from contextlib import AsyncExitStack
from datetime import datetime
from colorama import init, Fore, Style, Back

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client
from dotenv import load_dotenv

# Initialize colorama for cross-platform colored output
init(autoreset=True)

load_dotenv()  # Load environment variables from .env


class SequenceExecutionClient:
    def __init__(self):
        # Initialize session and client objects
        self.session: Optional[ClientSession] = None
        self.exit_stack = AsyncExitStack()
    
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
            
            print(f"{Fore.GREEN}✅ Connected successfully!{Style.RESET_ALL}")
            
        except Exception as e:
            print(f"{Fore.RED}❌ Failed to connect: {e}{Style.RESET_ALL}")
            raise
    
    def print_step_header(self, step_info: Dict[str, Any]):
        """Print a formatted header for a step"""
        step_num = step_info.get('step', '?')
        tool_name = step_info.get('tool_name', 'Unknown')
        status = step_info.get('status', 'pending')
        
        # Choose color based on status
        if status == 'executing':
            color = Fore.YELLOW
            icon = '⚡'
        elif status == 'success':
            color = Fore.GREEN
            icon = '✅'
        elif status == 'error':
            color = Fore.RED
            icon = '❌'
        else:
            color = Fore.CYAN
            icon = '⏳'
        
        print(f"\n{color}{'='*60}{Style.RESET_ALL}")
        print(f"{color}{icon} Step {step_num}: {tool_name} - {status.upper()}{Style.RESET_ALL}")
        print(f"{color}{'='*60}{Style.RESET_ALL}")
    
    def print_step_result(self, step_result: Dict[str, Any]):
        """Print detailed step result information"""
        # Timing information
        if 'started_at' in step_result:
            print(f"{Fore.BLUE}⏱️  Started:{Style.RESET_ALL} {step_result['started_at']}")
        if 'completed_at' in step_result:
            print(f"{Fore.BLUE}⏱️  Completed:{Style.RESET_ALL} {step_result['completed_at']}")
        if 'duration_ms' in step_result:
            print(f"{Fore.BLUE}⏱️  Duration:{Style.RESET_ALL} {step_result['duration_ms']}ms")
        
        # Progress information
        if 'progress' in step_result:
            print(f"{Fore.MAGENTA}📊 Progress:{Style.RESET_ALL} {step_result['progress']}")
        
        # Result or error
        if 'error' in step_result:
            print(f"\n{Fore.RED}❌ Error:{Style.RESET_ALL} {step_result['error']}")
        elif 'result' in step_result:
            result = step_result['result']
            if isinstance(result, dict):
                if 'content' in result:
                    print(f"\n{Fore.GREEN}📋 Result:{Style.RESET_ALL}")
                    # Pretty print the content
                    content = result['content']
                    if isinstance(content, list):
                        for item in content[:3]:  # Show first 3 items
                            print(f"   {json.dumps(item, indent=2)}")
                        if len(content) > 3:
                            print(f"   ... and {len(content) - 3} more items")
                    else:
                        print(f"   {json.dumps(content, indent=2)}")
    
    def print_execution_summary(self, summary: Dict[str, Any]):
        """Print the final execution summary"""
        print(f"\n{Fore.CYAN}{'='*60}{Style.RESET_ALL}")
        print(f"{Fore.CYAN}📊 EXECUTION SUMMARY{Style.RESET_ALL}")
        print(f"{Fore.CYAN}{'='*60}{Style.RESET_ALL}")
        
        if 'execution_summary' in summary:
            exec_summary = summary['execution_summary']
            print(f"{Fore.BLUE}Total Steps:{Style.RESET_ALL} {exec_summary.get('total_steps', 0)}")
            print(f"{Fore.GREEN}Successful:{Style.RESET_ALL} {exec_summary.get('successful_steps', 0)}")
            print(f"{Fore.RED}Failed:{Style.RESET_ALL} {exec_summary.get('failed_steps', 0)}")
            print(f"{Fore.YELLOW}Duration:{Style.RESET_ALL} {exec_summary.get('total_duration_ms', 0)}ms")
            
            if 'started_at' in exec_summary:
                print(f"{Fore.BLUE}Started:{Style.RESET_ALL} {exec_summary['started_at']}")
            if 'completed_at' in exec_summary:
                print(f"{Fore.BLUE}Completed:{Style.RESET_ALL} {exec_summary['completed_at']}")
    
    async def execute_sequence_demo(self):
        """Execute a demo sequence showing step-by-step progress"""
        if not self.session:
            raise RuntimeError("Not connected to MCP server")
        
        # Example sequence: Open Notepad, type text, save file
        sequence = [
            {
                "tool_name": "open_application",
                "arguments": {
                    "app_name": "notepad"
                }
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
                    "text_to_type": "Hello from MCP Execute Sequence!\n\nThis is a demonstration of step-by-step execution with detailed progress tracking.",
                    "clear_before_typing": false
                }
            },
            {
                "tool_name": "press_key_global",
                "arguments": {
                    "key_combination": "{Ctrl}s"
                }
            },
            {
                "tool_name": "delay",
                "arguments": {
                    "delay_ms": 1000
                }
            },
            {
                "tool_name": "type_into_element",
                "arguments": {
                    "selector": "edit|File name:",
                    "text_to_type": "mcp_sequence_demo.txt",
                    "clear_before_typing": true
                }
            },
            {
                "tool_name": "click_element",
                "arguments": {
                    "selector": "button|Save"
                }
            }
        ]
        
        print(f"\n{Fore.YELLOW}🚀 Starting Execute Sequence Demo{Style.RESET_ALL}")
        print(f"{Fore.YELLOW}This will open Notepad, type text, and save a file{Style.RESET_ALL}\n")
        
        # Convert sequence to JSON string for the tool
        tools_json = json.dumps(sequence)
        
        try:
            # Call the execute_sequence tool
            print(f"{Fore.CYAN}📡 Calling execute_sequence tool...{Style.RESET_ALL}")
            
            result = await self.session.call_tool(
                "execute_sequence",
                arguments={
                    "tools_json": tools_json,
                    "stop_on_error": True,
                    "include_detailed_results": True
                }
            )
            
            # Parse the result
            if result.content and len(result.content) > 0:
                # Extract the JSON content
                content = result.content[0]
                if hasattr(content, 'text'):
                    result_data = json.loads(content.text)
                else:
                    result_data = content
                
                # Print execution plan
                if 'execution_plan' in result_data:
                    print(f"\n{Fore.CYAN}📋 Execution Plan:{Style.RESET_ALL}")
                    plan = result_data['execution_plan']
                    for step in plan.get('steps', []):
                        print(f"   {step['step']}. {step['tool_name']} - {step.get('description', '')}")
                
                # Print step-by-step results
                if 'step_results' in result_data:
                    print(f"\n{Fore.CYAN}🔄 Step-by-Step Execution:{Style.RESET_ALL}")
                    for step_result in result_data['step_results']:
                        self.print_step_header(step_result)
                        self.print_step_result(step_result)
                        # Small delay to make output readable
                        await asyncio.sleep(0.5)
                
                # Print summary
                self.print_execution_summary(result_data)
                
                # Overall status
                status = result_data.get('status', 'unknown')
                if status == 'success':
                    print(f"\n{Fore.GREEN}🎉 Sequence completed successfully!{Style.RESET_ALL}")
                elif status == 'partial_success':
                    print(f"\n{Fore.YELLOW}⚠️  Sequence partially completed (stopped on error){Style.RESET_ALL}")
                else:
                    print(f"\n{Fore.RED}❌ Sequence completed with errors{Style.RESET_ALL}")
                
        except Exception as e:
            print(f"{Fore.RED}❌ Error executing sequence: {e}{Style.RESET_ALL}")
            raise
    
    async def cleanup(self):
        """Clean up resources"""
        try:
            await self.exit_stack.aclose()
        except (asyncio.CancelledError, Exception):
            # Ignore cleanup errors
            pass


async def main():
    """Main entry point"""
    client = SequenceExecutionClient()
    
    try:
        # Connect to the MCP server
        await client.connect_to_server()
        
        # Add a small delay to ensure connection is established
        await asyncio.sleep(1)
        
        # Run the demo
        await client.execute_sequence_demo()
        
    finally:
        # Clean up
        await client.cleanup()


if __name__ == "__main__":
    print(f"{Fore.CYAN}{'='*60}{Style.RESET_ALL}")
    print(f"{Fore.CYAN}🤖 MCP Execute Sequence Demo{Style.RESET_ALL}")
    print(f"{Fore.CYAN}{'='*60}{Style.RESET_ALL}")
    print(f"\nThis demo shows how the execute_sequence tool provides")
    print(f"detailed step-by-step execution information.\n")
    
    # Check if terminator-mcp-agent is available
    if not os.path.exists("target/release/terminator-mcp-agent"):
        print(f"{Fore.YELLOW}⚠️  Warning: terminator-mcp-agent not found in target/release/{Style.RESET_ALL}")
        print(f"Please build it first with: cargo build --release --bin terminator-mcp-agent\n")
    
    # Run the async main function
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print(f"\n{Fore.YELLOW}👋 Demo interrupted by user{Style.RESET_ALL}")