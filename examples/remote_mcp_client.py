#!/usr/bin/env python3
"""
Remote MCP Client for Terminator Desktop Automation
Connects to a remote Terminator MCP server via HTTP (e.g., through ngrok tunnel)
"""

import asyncio
import argparse
from contextlib import AsyncExitStack
from typing import Optional
from anthropic import Anthropic
from mcp import ClientSession
from mcp.client.session import transport_from_str
from mcp.shared.exceptions import McpError
import json
import os


class RemoteDesktopClient:
    def __init__(self, api_key: str):
        self.api_key = api_key
        self.anthropic = Anthropic(api_key=api_key)
        self.session: Optional[ClientSession] = None
        self.exit_stack = AsyncExitStack()
        
    async def connect_to_server(self, server_url: str):
        """Connect to the remote MCP server via HTTP"""
        print(f"🔌 Connecting to {server_url}...")
        
        # Add /mcp to the base URL if not present
        if not server_url.endswith('/mcp'):
            server_url = server_url.rstrip('/') + '/mcp'
        
        try:
            # Create HTTP transport
            transport = await transport_from_str(server_url)
            
            # Enter the transport context
            transport = await self.exit_stack.enter_async_context(transport)
            
            # Create and initialize session
            self.session = ClientSession(transport=transport)
            await self.exit_stack.enter_async_context(self.session)
            
            await self.session.initialize()
            
            # List available tools
            tools = await self.session.list_tools()
            print(f"✅ Connected! Available tools: {len(tools.tools)}")
            
            if tools.tools:
                print("   Available desktop automation tools:")
                for i, tool in enumerate(tools.tools[:5]):
                    print(f"   🔧 {tool.name}")
                if len(tools.tools) > 5:
                    print(f"   ... and {len(tools.tools) - 5} more")
                    
            return True
            
        except McpError as e:
            print(f"❌ MCP Error: {e}")
            return False
        except Exception as e:
            print(f"❌ Failed to connect: {e}")
            return False
    
    async def call_tool(self, tool_name: str, arguments: dict) -> dict:
        """Call a tool on the remote desktop"""
        if not self.session:
            raise RuntimeError("Not connected to server")
            
        try:
            result = await self.session.call_tool(tool_name, arguments)
            return result
        except McpError as e:
            print(f"❌ Tool error: {e}")
            raise
    
    async def automate_with_ai(self, prompt: str):
        """Use Claude to understand the request and execute desktop automation"""
        print(f"🤔 Thinking...")
        
        # Get available tools for context
        tools = await self.session.list_tools()
        tools_context = [
            {
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.inputSchema
            }
            for tool in tools.tools
        ]
        
        # Initialize conversation
        messages = [
            {"role": "user", "content": prompt}
        ]
        
        # Tool loop - keep calling Claude until no more tools are needed
        final_text = []
        
        while True:
            response = self.anthropic.messages.create(
                model="claude-opus-4-20250514",
                max_tokens=4000,
                temperature=0,
                messages=messages,
                tools=tools_context
            )
            
            # Process response
            assistant_message_content = []
            tool_calls_in_response = []
            
            for content in response.content:
                if content.type == 'text':
                    final_text.append(content.text)
                    assistant_message_content.append(content)
                elif content.type == 'tool_use':
                    tool_calls_in_response.append(content)
                    assistant_message_content.append(content)
            
            # If no tool calls, we're done
            if not tool_calls_in_response:
                break
            
            # Add assistant's message to conversation
            messages.append({
                "role": "assistant",
                "content": assistant_message_content
            })
            
            # Execute tool calls and collect results
            tool_results = []
            
            for tool_call in tool_calls_in_response:
                print(f"🔧 Calling tool: {tool_call.name}")
                if tool_call.input:
                    print(f"   Args: {tool_call.input}")
                
                try:
                    result = await self.call_tool(tool_call.name, tool_call.input)
                    
                    # Extract result content
                    result_content = []
                    if hasattr(result, 'content'):
                        for item in result.content:
                            if hasattr(item, 'text'):
                                result_content.append(item.text)
                            elif hasattr(item, 'type') and item.type == 'image':
                                result_content.append("[Screenshot captured]")
                    
                    result_text = "\n".join(result_content) if result_content else "Tool executed successfully"
                    
                    tool_results.append({
                        "type": "tool_result",
                        "tool_use_id": tool_call.id,
                        "content": result_text
                    })
                    
                    print(f"   ✅ Result: {result_text[:100]}..." if len(result_text) > 100 else f"   ✅ Result: {result_text}")
                    
                except Exception as e:
                    error_msg = f"Error executing tool: {str(e)}"
                    print(f"   ❌ {error_msg}")
                    tool_results.append({
                        "type": "tool_result",
                        "tool_use_id": tool_call.id,
                        "content": error_msg
                    })
            
            # Add tool results to conversation
            messages.append({
                "role": "user",
                "content": tool_results
            })
        
        # Print final response
        if final_text:
            print(f"\n🤖 Claude: {' '.join(final_text)}")
    
    async def cleanup(self):
        """Clean up resources"""
        try:
            await self.exit_stack.aclose()
        except:
            pass


async def interactive_mode(client: RemoteDesktopClient):
    """Interactive mode for desktop control"""
    print("\n🤖 Remote Desktop Control")
    print("=" * 50)
    print("You can now control the remote desktop using natural language!")
    print("Examples:")
    print("  - 'Open Notepad and type Hello World'")
    print("  - 'Take a screenshot of the desktop'")
    print("  - 'Show me all running applications'")
    print("  - 'Click on the Start button'")
    print("\nType 'exit' or 'quit' to end the session.")
    print("=" * 50)
    
    while True:
        try:
            user_input = input("\n💬 You: ").strip()
            
            if user_input.lower() in ['exit', 'quit', 'q']:
                print("👋 Goodbye!")
                break
                
            if not user_input:
                continue
                
            await client.automate_with_ai(user_input)
            
        except KeyboardInterrupt:
            print("\n👋 Goodbye!")
            break
        except Exception as e:
            print(f"❌ Error: {e}")


async def main():
    parser = argparse.ArgumentParser(description='Remote MCP Client for Desktop Automation')
    parser.add_argument(
        'server_url',
        help='The HTTP URL of the remote MCP server (e.g., https://abc123.ngrok-free.app or http://localhost:3000)'
    )
    parser.add_argument(
        '--api-key',
        default=None,
        help='Anthropic API key (or set ANTHROPIC_API_KEY env var)'
    )
    parser.add_argument(
        '--command',
        help='Single command to execute (non-interactive mode)'
    )
    
    args = parser.parse_args()
    
    # Get API key
    api_key = args.api_key or os.getenv('ANTHROPIC_API_KEY')
    if not api_key:
        print("❌ Error: Anthropic API key required")
        print("Set ANTHROPIC_API_KEY environment variable or use --api-key")
        return
    
    # Create client
    client = RemoteDesktopClient(api_key)
    
    try:
        # Connect to server
        if await client.connect_to_server(args.server_url):
            if args.command:
                # Execute single command
                await client.automate_with_ai(args.command)
            else:
                # Interactive mode
                await interactive_mode(client)
    finally:
        await client.cleanup()


if __name__ == "__main__":
    asyncio.run(main()) 