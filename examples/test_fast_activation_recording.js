#!/usr/bin/env node
/**
 * Fast Activation Recording Test
 * 
 * Tests the optimized activate_element generation by:
 * 1. Starting a recording session
 * 2. Switching between applications (to trigger ApplicationSwitchEvent)
 * 3. Stopping recording and analyzing the generated MCP workflow
 * 4. Verifying that activate_element steps have optimizations applied
 * 
 * Usage:
 *   node test_fast_activation_recording.js
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

class FastActivationRecordingTest {
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
      throw new Error('❌ MCP binary not found. Build with: cargo build --release --bin terminator-mcp-agent');
    }
    
    console.log(`📁 Using binary: ${binaryPath}`);
    
    // Start the server process
    this.serverProcess = spawn(binaryPath, [
      '--transport', 'http',
      '--port', port.toString()
    ], {
      stdio: ['ignore', 'pipe', 'pipe'],
      env: {
        ...process.env,
        RUST_LOG: 'info',
        RUST_BACKTRACE: '1'
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
      this.transport = new StreamableHTTPClientTransport(new URL(httpUrl));
      this.client = new Client(
        {
          name: "fast-activation-test",
          version: "1.0.0",
        },
        {
          capabilities: {
            tools: {},
          },
        }
      );
      
      await this.client.connect(this.transport);
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

  analyzeActivationStep(step, index) {
    console.log(`\n📋 Step ${index + 1}: ${step.tool_name}`);
    console.log(`   Description: ${step.description}`);
    
    if (step.tool_name === 'activate_element') {
      console.log('   🔍 Analyzing activate_element optimizations:');
      
      const args = step.arguments;
      const optimizations = [];
      const warnings = [];
      
      // Check for speed optimizations
      if (args.include_tree === false) {
        optimizations.push('✅ include_tree: false (skips UI tree building)');
      } else {
        warnings.push('⚠️  include_tree not disabled (may be slow)');
      }
      
      if (args.timeout_ms && args.timeout_ms <= 1000) {
        optimizations.push(`✅ timeout_ms: ${args.timeout_ms} (fast timeout)`);
      } else if (args.timeout_ms) {
        warnings.push(`⚠️  timeout_ms: ${args.timeout_ms} (may be slow)`);
      } else {
        warnings.push('⚠️  timeout_ms not set (will use 3000ms default)');
      }
      
      if (args.retries === 0) {
        optimizations.push('✅ retries: 0 (no retry loops)');
      } else if (args.retries) {
        warnings.push(`⚠️  retries: ${args.retries} (may add delays)`);
      }
      
      if (args.fallback_selectors) {
        optimizations.push(`✅ fallback_selectors: ${args.fallback_selectors} (reliable)`);
      }
      
      if (step.delay_ms && step.delay_ms <= 200) {
        optimizations.push(`✅ delay_ms: ${step.delay_ms} (fast execution)`);
      } else if (step.delay_ms) {
        warnings.push(`⚠️  delay_ms: ${step.delay_ms} (may be slow)`);
      }
      
      // Display results
      if (optimizations.length > 0) {
        console.log('   🚀 Applied optimizations:');
        optimizations.forEach(opt => console.log(`     ${opt}`));
      }
      
      if (warnings.length > 0) {
        console.log('   ⚠️  Potential performance issues:');
        warnings.forEach(warn => console.log(`     ${warn}`));
      }
      
      // Calculate estimated time savings
      const baseTime = 3000 + 500 + 1000; // Default timeout + verification + delay
      let optimizedTime = (args.timeout_ms || 3000) + 500 + (step.delay_ms || 1000);
      if (args.include_tree !== false) {
        optimizedTime += 2000; // Estimated tree building time
      }
      
      const savings = baseTime - optimizedTime + (args.include_tree === false ? 2000 : 0);
      if (savings > 0) {
        console.log(`   ⏱️  Estimated time savings: ~${savings}ms`);
      }
      
      return { optimizations: optimizations.length, warnings: warnings.length, savings };
    }
    
    return null;
  }

  async testFastActivationRecording() {
    console.log('\n' + '='.repeat(60));
    console.log('🚀 FAST ACTIVATION RECORDING TEST');
    console.log('='.repeat(60));
    console.log('');
    console.log('This test will record for 10 seconds to capture app switches.');
    console.log('Please perform the following actions:');
    console.log('• Switch between applications (Alt+Tab, taskbar clicks, etc.)');
    console.log('• Click on different windows');
    console.log('• Open/focus apps like Chrome, Notepad, Calculator, etc.');
    console.log('');
    console.log('The test will analyze the generated activate_element steps');
    console.log('to verify performance optimizations are applied.');
    console.log('');
    
    try {
      // Step 1: Start recording
      console.log('📹 Starting optimized recording session...');
      const startResult = await this.callTool('record_workflow', {
        action: 'start',
        workflow_name: 'fast_activation_test',
        low_energy_mode: false,
        record_scroll_events: false, // Focus on app switches
        highlight_mode: {
          enabled: true,
          duration_ms: 1000,
          color: 0x0000FF, // Red border
          show_labels: true,
          label_position: 'Top',
          label_style: {
            size: 12,
            bold: true,
            color: 0xFFFFFF
          }
        }
      });
      
      console.log('🎬 Recording started with highlighting!');
      console.log('🔴 Look for RED borders showing captured events');
      console.log('👉 Now switch between applications...');
      console.log('');
      
      // Step 2: Wait 10 seconds with countdown
      for (let i = 10; i > 0; i--) {
        process.stdout.write(`\r⏳ Recording... ${i} seconds remaining`);
        await setTimeout(1000);
      }
      console.log('\r⏱️  10 seconds completed!                    ');
      console.log('');
      
      // Step 3: Stop recording
      console.log('⏹️  Stopping recording...');
      const stopResult = await this.callTool('record_workflow', {
        action: 'stop'
      });
      
      console.log('Recording stopped!');
      
      // Step 4: Analyze results
      if (stopResult && stopResult.length > 0) {
        const result = stopResult[0];
        if (result.type === 'text') {
          const data = JSON.parse(result.text);
          console.log('\n🎉 RECORDING RESULTS:');
          console.log(`   Workflow name: ${data.workflow_name}`);
          console.log(`   File path: ${data.file_path}`);
          console.log(`   Status: ${data.status}`);
          
          if (data.mcp_workflow && data.mcp_workflow.arguments && data.mcp_workflow.arguments.items) {
            const steps = data.mcp_workflow.arguments.items;
            console.log(`\n📋 Generated MCP Workflow (${steps.length} steps):`);
            
            let totalOptimizations = 0;
            let totalWarnings = 0;
            let totalSavings = 0;
            let activationSteps = 0;
            
            steps.forEach((step, index) => {
              const analysis = this.analyzeActivationStep(step, index);
              if (analysis) {
                activationSteps++;
                totalOptimizations += analysis.optimizations;
                totalWarnings += analysis.warnings;
                totalSavings += analysis.savings;
              } else {
                console.log(`\n📋 Step ${index + 1}: ${step.tool_name}`);
                console.log(`   Description: ${step.description}`);
              }
            });
            
            // Summary
            console.log('\n' + '='.repeat(50));
            console.log('📊 OPTIMIZATION ANALYSIS SUMMARY');
            console.log('='.repeat(50));
            console.log(`Total steps: ${steps.length}`);
            console.log(`activate_element steps: ${activationSteps}`);
            console.log(`Applied optimizations: ${totalOptimizations}`);
            console.log(`Performance warnings: ${totalWarnings}`);
            if (totalSavings > 0) {
              console.log(`Estimated total time savings: ~${totalSavings}ms`);
            }
            
            if (activationSteps === 0) {
              console.log('\n⚠️  No application switches were captured.');
              console.log('   Try switching between apps more clearly during recording.');
            } else if (totalOptimizations > totalWarnings) {
              console.log('\n✅ Optimization SUCCESS! Fast activate_element steps generated.');
            } else {
              console.log('\n⚠️  Some optimizations may be missing. Check implementation.');
            }
            
          } else {
            console.log('\n⚠️  No MCP workflow generated or workflow is empty');
          }
          
          // Show raw events for debugging
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
            }
          }
        }
      }
      
    } catch (error) {
      console.error('❌ Fast activation recording test failed:', error);
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
  console.log('🧪 MCP Fast Activation Recording Test');
  console.log('Testing optimized activate_element generation\n');
  
  const client = new FastActivationRecordingTest();
  
  try {
    // Start the MCP server
    await client.startMcpServer(3001);
    
    // Connect to the server
    await client.connect(3001);
    
    // Run the test
    await client.testFastActivationRecording();
    
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

export { FastActivationRecordingTest };
