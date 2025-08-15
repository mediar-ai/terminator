#!/usr/bin/env python3
"""
Natural Language MCP Client for Terminator Desktop Automation (HTTP Transport)
Uses Claude to understand natural language and execute desktop automation via HTTP

Note: This requires the MCP server to support streamable-http transport.
Currently, the terminator-mcp-agent only supports stdio transport.
This client is provided for future use or custom server implementations.
"""

import asyncio
import os
from typing import Optional, List, Dict, Any
from contextlib import AsyncExitStack

from mcp import ClientSession
from mcp.client.streamable_http import streamablehttp_client

from anthropic import Anthropic
from dotenv import load_dotenv

load_dotenv()  # Load environment variables from .env


class NaturalLanguageMCPClient:
    def __init__(self):
        # Initialize session and client objects
        self.session: Optional[ClientSession] = None
        self.exit_stack = AsyncExitStack()
        
        # Initialize Anthropic client
        api_key = os.getenv("ANTHROPIC_API_KEY")
        if not api_key:
            raise ValueError("ANTHROPIC_API_KEY environment variable is required")
        self.anthropic = Anthropic(api_key=api_key)
    
    async def connect_to_server(self, server_url: str = "http://localhost:3000/mcp"):
        """Connect to the terminator MCP server via HTTP"""
        try:
            print(f"🔌 Connecting to {server_url}...")
            
            # Create the client transport and connect
            transport = await self.exit_stack.enter_async_context(
                streamablehttp_client(server_url)
            )
            
            # Create the session
            self.session = await self.exit_stack.enter_async_context(
                ClientSession(transport[0], transport[1])
            )
            
            # Initialize the connection
            await self.session.initialize()
            
            # List available tools
            tools_result = await self.session.list_tools()
            print(f"✅ Connected! Available tools: {len(tools_result.tools)}")
            for tool in tools_result.tools[:5]:  # Show first 5 tools
                print(f"   🔧 {tool.name}")
            if len(tools_result.tools) > 5:
                print(f"   ... and {len(tools_result.tools) - 5} more")
            
        except Exception as e:
            print(f"❌ Failed to connect: {e}")
            raise
    
    async def process_query(self, query: str) -> str:
        """Process a natural language query using Claude and MCP tools"""
        if not self.session:
            raise RuntimeError("Not connected to MCP server")
        
        # Get available tools
        tools_result = await self.session.list_tools()
        available_tools = []
        
        # Convert MCP tools to Anthropic format
        for tool in tools_result.tools:
            available_tools.append({
                "name": tool.name,
                "description": tool.description or "",
                "input_schema": tool.inputSchema
            })
        
        # Initialize conversation with user query
        messages = [
            {
                "role": "user",
                "content": query
            }
        ]
        
        # Initial Claude API call
        response = self.anthropic.messages.create(
            model="claude-opus-4-20250514",
            max_tokens=1000,
            messages=messages,
            tools=available_tools
        )
        
        # Process response and handle tool calls in a loop
        final_text = []
        
        while True:
            assistant_message_content = []
            tool_calls_in_response = []
            
            for content in response.content:
                if content.type == 'text':
                    final_text.append(content.text)
                    assistant_message_content.append(content)
                elif content.type == 'tool_use':
                    tool_calls_in_response.append(content)
                    assistant_message_content.append(content)
            
            # If there are no tool calls, we're done
            if not tool_calls_in_response:
                break
            
            # Add the assistant's message (with tool calls) to the conversation
            messages.append({
                "role": "assistant",
                "content": assistant_message_content
            })
            
            # Execute all tool calls and collect results
            tool_results = []
            
            for tool_call in tool_calls_in_response:
                tool_name = tool_call.name
                tool_args = tool_call.input
                
                print(f"\n🔧 Calling tool: {tool_name}")
                if tool_args:
                    print(f"   Args: {tool_args}")
                
                try:
                    # Execute tool call
                    result = await self.session.call_tool(tool_name, arguments=tool_args)
                    
                    # Extract the result content
                    result_content = []
                    for item in result.content:
                        if hasattr(item, 'text'):
                            result_content.append(item.text)
                        elif hasattr(item, 'data'):
                            result_content.append(str(item.data))
                    
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
            
            # Add tool results to the conversation
            messages.append({
                "role": "user",
                "content": tool_results
            })
            
            # Get next response from Claude
            response = self.anthropic.messages.create(
                model="claude-opus-4-20250514",
                max_tokens=1000,
                messages=messages,
                tools=available_tools
            )
        
        return "\n".join(final_text)
    
    async def chat_loop(self):
        """Run an interactive chat session"""
        print("\n🤖 Natural Language Desktop Control (via HTTP)")
        print("=" * 50)
        print("You can now control your desktop using natural language!")
        print("Examples:")
        print("  - 'Open Notepad and type Hello World'")
        print("  - 'Take a screenshot of the desktop'")
        print("  - 'Show me all running applications'")
        print("  - 'Click on the Start button'")
        print("\nType 'exit' or 'quit' to end the session.")
        print("=" * 50)
        
        while True:
            try:
                # Get user input
                user_input = input("\n💬 You: ").strip()
                
                if user_input.lower() in ['exit', 'quit']:
                    print("\n👋 Goodbye!")
                    break
                
                if not user_input:
                    continue
                
                # Process the query
                print("\n🤔 Thinking...")
                response = await self.process_query(user_input)
                
                print(f"\n🤖 Claude: {response}")
                
            except KeyboardInterrupt:
                print("\n\n👋 Goodbye!")
                break
            except Exception as e:
                print(f"\n❌ Error: {e}")
    
    async def cleanup(self):
        """Clean up resources"""
        await self.exit_stack.aclose()


async def main():
    """Main entry point"""
    import argparse
    
    parser = argparse.ArgumentParser(description="Natural Language MCP Client (HTTP Transport)")
    parser.add_argument(
        "--server-url",
        default="http://localhost:3000/mcp",
        help="URL of the MCP HTTP server (default: http://localhost:3000/mcp)"
    )
    args = parser.parse_args()
    
    client = NaturalLanguageMCPClient()
    
    try:
        # Connect to the MCP server
        await client.connect_to_server(args.server_url)
        
        # Run the interactive chat loop
        await client.chat_loop()
        
    finally:
        # Clean up
        await client.cleanup()


if __name__ == "__main__":
    # Check for API key
    if not os.getenv("ANTHROPIC_API_KEY"):
        print("❌ Error: ANTHROPIC_API_KEY environment variable is required")
        print("Please set it in your .env file or export it:")
        print("  export ANTHROPIC_API_KEY='your-api-key-here'")
        exit(1)
    
    print("⚠️  Note: This client requires a server that supports streamable-http transport.")
    print("The current terminator-mcp-agent only supports stdio transport.")
    print("This client is provided for future use or custom server implementations.")
    print()
    
    # Run the async main function
    asyncio.run(main()) 