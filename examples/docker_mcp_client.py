#!/usr/bin/env python3
"""
Docker MCP Client for Terminator Desktop Automation
Connects to Terminator MCP Agent running in a Docker Windows container via HTTP

Prerequisites:
pip install mcp anthropic python-dotenv

Usage:
python docker_mcp_client.py --server-url http://localhost:8080/mcp
"""

import asyncio
import os
import argparse
from typing import Optional, List, Dict, Any
from contextlib import AsyncExitStack

from mcp import ClientSession
from mcp.client.streamable_http import streamablehttp_client

from anthropic import Anthropic
from dotenv import load_dotenv

load_dotenv()  # Load environment variables from .env


class DockerMCPClient:
    def __init__(self, server_url: str = "http://localhost:8080/mcp"):
        # Initialize session and client objects
        self.session: Optional[ClientSession] = None
        self.exit_stack = AsyncExitStack()
        self.server_url = server_url
        
        # Initialize Anthropic client (optional - for AI-powered automation)
        api_key = os.getenv("ANTHROPIC_API_KEY")
        if api_key:
            self.anthropic = Anthropic(api_key=api_key)
            self.ai_enabled = True
        else:
            self.anthropic = None
            self.ai_enabled = False
            print("⚠️  ANTHROPIC_API_KEY not found - AI features disabled")
    
    async def connect_to_server(self):
        """Connect to the terminator MCP server via HTTP"""
        try:
            print(f"🔌 Connecting to Docker MCP server at {self.server_url}...")
            
            # Create the client transport and connect
            transport = await self.exit_stack.enter_async_context(
                streamablehttp_client(self.server_url)
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
            print("💡 Make sure the Docker container is running:")
            print("   docker-compose -f docker/docker-compose.windows.yml up")
            raise
    
    async def test_basic_functionality(self):
        """Test basic MCP functionality"""
        if not self.session:
            raise RuntimeError("Not connected to MCP server")
        
        print("\n🧪 Testing basic functionality...")
        
        try:
            # Test 1: Get applications
            print("📱 Getting applications...")
            result = await self.session.call_tool("get_applications", arguments={})
            apps_text = "\n".join([item.text for item in result.content if hasattr(item, 'text')])
            print(f"   Found applications: {apps_text[:100]}...")
            
            # Test 2: Get windows
            print("🪟 Getting windows...")
            result = await self.session.call_tool("get_windows", arguments={})
            windows_text = "\n".join([item.text for item in result.content if hasattr(item, 'text')])
            print(f"   Found windows: {windows_text[:100]}...")
            
            # Test 3: Get focused window tree (if any window is focused)
            print("🌳 Getting focused window tree...")
            try:
                result = await self.session.call_tool("get_focused_window_tree", arguments={})
                tree_text = "\n".join([item.text for item in result.content if hasattr(item, 'text')])
                print(f"   Window tree: {tree_text[:100]}...")
            except Exception as e:
                print(f"   No focused window or error: {e}")
            
            print("✅ Basic functionality test completed!")
            
        except Exception as e:
            print(f"❌ Basic functionality test failed: {e}")
    
    async def process_ai_query(self, query: str) -> str:
        """Process a natural language query using Claude and MCP tools"""
        if not self.session:
            raise RuntimeError("Not connected to MCP server")
        
        if not self.ai_enabled:
            print("❌ AI features not available - ANTHROPIC_API_KEY required")
            return "AI features disabled"
        
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
                "content": f"I'm connected to a Terminator MCP agent running in a Docker Windows container. {query}"
            }
        ]
        
        # Initial Claude API call
        response = self.anthropic.messages.create(
            model="claude-3-5-sonnet-20241022",
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
                model="claude-3-5-sonnet-20241022",
                max_tokens=1000,
                messages=messages,
                tools=available_tools
            )
        
        return "\n".join(final_text)
    
    async def interactive_mode(self):
        """Run an interactive session"""
        print("\n🤖 Docker MCP Client - Interactive Mode")
        print("=" * 60)
        print("Connected to Terminator MCP Agent in Docker Windows container")
        print("Available commands:")
        print("  - 'test' - Run basic functionality tests")
        print("  - 'apps' - List applications")
        print("  - 'windows' - List windows")
        print("  - 'tree' - Get focused window tree")
        if self.ai_enabled:
            print("  - Any natural language query (AI-powered)")
        print("  - 'exit' or 'quit' - End session")
        print("=" * 60)
        
        while True:
            try:
                # Get user input
                user_input = input("\n💬 You: ").strip()
                
                if user_input.lower() in ['exit', 'quit']:
                    print("\n👋 Goodbye!")
                    break
                
                if not user_input:
                    continue
                
                # Handle special commands
                if user_input.lower() == 'test':
                    await self.test_basic_functionality()
                    continue
                
                if user_input.lower() == 'apps':
                    result = await self.session.call_tool("get_applications", arguments={})
                    apps_text = "\n".join([item.text for item in result.content if hasattr(item, 'text')])
                    print(f"\n📱 Applications:\n{apps_text}")
                    continue
                
                if user_input.lower() == 'windows':
                    result = await self.session.call_tool("get_windows", arguments={})
                    windows_text = "\n".join([item.text for item in result.content if hasattr(item, 'text')])
                    print(f"\n🪟 Windows:\n{windows_text}")
                    continue
                
                if user_input.lower() == 'tree':
                    try:
                        result = await self.session.call_tool("get_focused_window_tree", arguments={})
                        tree_text = "\n".join([item.text for item in result.content if hasattr(item, 'text')])
                        print(f"\n🌳 Focused Window Tree:\n{tree_text}")
                    except Exception as e:
                        print(f"\n❌ Error getting window tree: {e}")
                    continue
                
                # Process as AI query if AI is enabled
                if self.ai_enabled:
                    print("\n🤔 Processing with AI...")
                    response = await self.process_ai_query(user_input)
                    print(f"\n🤖 Claude: {response}")
                else:
                    print("\n❌ AI features not available. Use specific commands like 'apps', 'windows', 'tree', or 'test'")
                
            except KeyboardInterrupt:
                print("\n\n👋 Goodbye!")
                break
            except Exception as e:
                print(f"\n❌ Error: {e}")
    
    async def cleanup(self):
        """Clean up resources"""
        try:
            await self.exit_stack.aclose()
        except (asyncio.CancelledError, Exception):
            # Ignore cleanup errors - they often happen on exit
            pass


async def main():
    """Main entry point"""
    parser = argparse.ArgumentParser(description="Docker MCP Client for Terminator")
    parser.add_argument(
        "--server-url",
        default="http://localhost:8080/mcp",
        help="URL of the MCP HTTP server (default: http://localhost:8080/mcp)"
    )
    parser.add_argument(
        "--test-only",
        action="store_true",
        help="Run basic tests and exit"
    )
    args = parser.parse_args()
    
    client = DockerMCPClient(args.server_url)
    
    try:
        # Connect to the MCP server
        await client.connect_to_server()
        
        if args.test_only:
            # Run tests and exit
            await client.test_basic_functionality()
        else:
            # Run the interactive session
            await client.interactive_mode()
        
    finally:
        # Clean up
        await client.cleanup()


if __name__ == "__main__":
    # Run the async main function
    asyncio.run(main())
