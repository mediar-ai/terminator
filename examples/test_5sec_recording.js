#!/usr/bin/env node
/**
 * Simple 5-Second Recording Test
 * 
 * Tests the terminator MCP agent's record_workflow functionality by:
 * 1. Connecting to the MCP server via HTTP
 * 2. Starting a workflow recording
 * 3. Waiting 5 seconds for manual interactions
 * 4. Stopping the recording and showing results
 * 
 * Usage:
 *   node test_5sec_recording.js
 */

import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';
import { spawn } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import { setTimeout } from 'timers/promises';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

class Simple5SecRecordingClient {
  constructor() {
    this.client = null;
    this.transport = null;
    this.serverProcess = null;
  }

  async startMcpServer(port = 3001) {
    console.log(`🚀 Starting MCP server on port ${port}...`);
    
    // Find the MCP binary
    const possiblePaths = [
      path.join(__dirname, '../target/release/terminator-mcp-agent.exe'),
      path.join(__dirname, '../target/release/terminator-mcp-agent'),
      'target/release/terminator-mcp-agent.exe',
      'target/release/terminator-mcp-agent',
    ];
    
    let binaryPath = null;
    for (const p of possiblePaths) {
      if (fs.existsSync(p)) {
        binaryPath = p;
        break;
      }
    }
    
    if (!binaryPath) {
      throw new Error('❌ MCP binary not found. Build with: cargo build --release');
    }
    
    console.log(`📁 Using binary: ${binaryPath}`);
    
    // Start the server process with debug logging
    this.serverProcess = spawn(binaryPath, [
      '--transport', 'http',
      '--port', port.toString()
    ], {
      stdio: ['ignore', 'pipe', 'pipe'],
      env: {
        ...process.env,
        RUST_LOG: 'debug', // Enable debug logging
        RUST_BACKTRACE: '1' // Enable backtraces
      }
    });
    
    // Log server output for debugging
    this.serverProcess.stdout?.on('data', (data) => {
      console.log(`[SERVER] ${data.toString().trim()}`);
    });
    
    this.serverProcess.stderr?.on('data', (data) => {
      console.error(`[SERVER ERROR] ${data.toString().trim()}`);
    });
    
    this.serverProcess.on('exit', (code) => {
      console.log(`[SERVER] Process exited with code ${code}`);
    });
    
    // Wait for server to start
    console.log('⏳ Waiting for server to initialize...');
    await setTimeout(3000);
    
    // Test server health
    try {
      const healthUrl = `http://127.0.0.1:${port}/health`;
      const response = await fetch(healthUrl, {
        method: 'GET',
        signal: AbortSignal.timeout(5000)
      });
      
      if (!response.ok) {
        throw new Error(`Health check failed: ${response.status}`);
      }
      console.log('✅ Server health check passed');
    } catch (error) {
      throw new Error(`Cannot reach MCP server: ${error}`);
    }
  }

  async connect(port = 3001) {
    const httpUrl = `http://127.0.0.1:${port}/mcp`;
    console.log(`🔌 Connecting to MCP server at ${httpUrl}...`);
    
    try {
      // Create StreamableHTTP transport
      this.transport = new StreamableHTTPClientTransport(new URL(httpUrl));
      
      // Create MCP client
      this.client = new Client(
        {
          name: "5sec-recording-test",
          version: "1.0.0",
        },
        {
          capabilities: {
            tools: {},
          },
        }
      );
      
      // Connect to the server
      await this.client.connect(this.transport);
      
      // Wait for connection to stabilize
      await setTimeout(500);
      
      console.log('✅ Connected to MCP server');
    } catch (error) {
      console.error('❌ Failed to connect:', error);
      throw error;
    }
  }

  async callTool(name, arguments_) {
    if (!this.client) {
      throw new Error('MCP client not connected');
    }
    
    console.log(`🛠️  Calling tool: ${name}`);
    if (arguments_ && Object.keys(arguments_).length > 0) {
      console.log(`   Arguments:`, JSON.stringify(arguments_, null, 2));
    }
    
    try {
      const result = await this.client.callTool({
        name,
        arguments: arguments_ || {},
      });
      
      console.log(`✅ Tool ${name} completed successfully`);
      return result.content;
    } catch (error) {
      console.error(`❌ Tool ${name} failed:`, error);
      throw error;
    }
  }

  async test5SecondRecording() {
    console.log('\n' + '='.repeat(60));
    console.log('⏱️  5-SECOND RECORDING TEST');
    console.log('='.repeat(60));
    console.log('');
    console.log('This will start recording for 5 seconds.');
    console.log('During this time, you can:');
    console.log('• Click on things');
    console.log('• Type text');
    console.log('• Switch between windows');
    console.log('• Or just let it record idle state');
    console.log('');
    
    try {
      // Step 1: Start recording with highlighting enabled
      console.log('📹 Starting 5-second recording with visual highlighting...');
      const startResult = await this.callTool('record_workflow', {
        action: 'start',
        workflow_name: '5sec_test_recording',
        low_energy_mode: false,
        highlight_mode: {
          enabled: true,
          duration_ms: 500,        // 500ms highlight duration
          color: 0x0000FF,         // Red border (BGR format)
          show_labels: true,       // Show event type labels
          label_position: 'Top',   // Labels at top
          label_style: {
            size: 14,
            bold: true,
            color: 0xFFFFFF        // White text
          }
        }
      });
      
      console.log('🎬 Recording started with visual highlighting!');
      
      // Check if highlighting was actually enabled in the response
      if (startResult && startResult.length > 0 && startResult[0].type === 'text') {
        const startData = JSON.parse(startResult[0].text);
        if (startData.highlighting_enabled) {
          console.log('✅ Visual highlighting is ACTIVE');
          console.log(`   • Color: 0x${startData.highlight_color.toString(16).padStart(6, '0').toUpperCase()} (red border)`);
          console.log(`   • Duration: ${startData.highlight_duration_ms}ms per event`);
        }
      }
      
      console.log('🔴 Look for RED borders with event labels (CLICK, TYPE, etc.) on UI elements');
      console.log('👉 Perform any actions you want to capture...');
      console.log('');
      
      // Step 2: Wait exactly 5 seconds with countdown
      for (let i = 5; i > 0; i--) {
        process.stdout.write(`\r⏳ Recording... ${i} seconds remaining`);
        await setTimeout(1000);
      }
      console.log('\r⏱️  5 seconds completed!                    ');
      console.log('');
      
      // Step 3: Stop recording
      console.log('⏹️  Stopping recording...');
      const stopResult = await this.callTool('record_workflow', {
        action: 'stop'
      });
      
      console.log('Recording stopped!');
      
      // Display results
      if (stopResult && stopResult.length > 0) {
        const result = stopResult[0];
        if (result.type === 'text') {
          const data = JSON.parse(result.text);
          console.log('\n🎉 RECORDING RESULTS:');
          console.log(`   Workflow name: ${data.workflow_name}`);
          console.log(`   File path: ${data.file_path}`);
          console.log(`   Status: ${data.status}`);
          
          if (data.mcp_workflow) {
            console.log('\n📋 Generated MCP Workflow:');
            const workflow = data.mcp_workflow;
            if (workflow.arguments && workflow.arguments.items) {
              console.log(`   Total steps: ${workflow.arguments.items.length}`);
              console.log('   Steps:');
              workflow.arguments.items.forEach((step, index) => {
                console.log(`     ${index + 1}. ${step.tool_name} - ${JSON.stringify(step.arguments).substring(0, 80)}...`);
              });
            } else {
              console.log(JSON.stringify(workflow, null, 2));
            }
          } else {
            console.log('   No MCP workflow generated (no capturable events detected)');
          }
          
          if (data.file_content) {
            const content = JSON.parse(data.file_content);
            console.log(`\n📊 Raw Events Captured: ${content.events ? content.events.length : 0}`);
            if (content.events && content.events.length > 0) {
              console.log('   Event types:');
              const eventTypes = {};
              content.events.forEach(event => {
                const type = Object.keys(event.event)[0];
                eventTypes[type] = (eventTypes[type] || 0) + 1;
              });
              Object.entries(eventTypes).forEach(([type, count]) => {
                console.log(`     - ${type}: ${count} events`);
              });
              
              console.log('\n📄 First few events:');
              content.events.slice(0, 3).forEach((event, index) => {
                const eventType = Object.keys(event.event)[0];
                console.log(`     ${index + 1}. [${event.timestamp}] ${eventType}`);
              });
            }
          }
        }
      }
      
    } catch (error) {
      console.error('❌ 5-second recording test failed:', error);
      throw error;
    }
  }

  async cleanup() {
    console.log('\n🧹 Cleaning up...');
    
    try {
      if (this.client) {
        await this.client.close();
        this.client = null;
      }
      
      if (this.transport) {
        await this.transport.close();
        this.transport = null;
      }
      
      if (this.serverProcess) {
        console.log('🛑 Stopping MCP server...');
        this.serverProcess.kill('SIGTERM');
        
        // Wait for graceful shutdown
        await new Promise((resolve) => {
          const timeoutId = globalThis.setTimeout(() => {
            console.log('⚠️  Force killing server process...');
            this.serverProcess?.kill('SIGKILL');
            resolve();
          }, 5000);
          
          this.serverProcess?.on('exit', () => {
            globalThis.clearTimeout(timeoutId);
            resolve();
          });
        });
        
        this.serverProcess = null;
      }
      
      console.log('✅ Cleanup completed');
      
    } catch (error) {
      console.error('⚠️  Error during cleanup:', error);
    }
  }
}

async function main() {
  console.log('🧪 MCP 5-Second Recording Test');
  console.log('Simple workflow recording test - no automated actions\n');
  
  const client = new Simple5SecRecordingClient();
  
  try {
    // Start the MCP server
    await client.startMcpServer(3001);
    
    // Connect to the server
    await client.connect(3001);
    
    // Run the 5-second recording test
    await client.test5SecondRecording();
    
    console.log('\n🎉 Test completed successfully!');
    
  } catch (error) {
    console.error('\n💥 Test failed:', error);
    process.exit(1);
  } finally {
    await client.cleanup();
  }
}

// Handle process signals for cleanup
process.on('SIGINT', async () => {
  console.log('\n⚠️  Received SIGINT, cleaning up...');
  process.exit(0);
});

process.on('SIGTERM', async () => {
  console.log('\n⚠️  Received SIGTERM, cleaning up...');
  process.exit(0);
});

// Run the test
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch(error => {
    console.error('💥 Unhandled error:', error);
    process.exit(1);
  });
}

export { Simple5SecRecordingClient };
