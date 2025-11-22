/**
 * Terminator MCP Client
 * 
 * Provides a client interface to interact with the Terminator MCP server
 * for desktop automation tasks like clicking elements, typing text, etc.
 */

import { spawn } from 'child_process';
import { EventEmitter } from 'events';

export class TerminatorMCPClient extends EventEmitter {
    constructor() {
        super();
        this.process = null;
        this.messageId = 0;
        this.pendingRequests = new Map();
        this.buffer = '';
        this.initialized = false;
    }

    /**
     * Start the Terminator MCP server
     */
    async start() {
        return new Promise((resolve, reject) => {
            console.log('Starting Terminator MCP server...');

            // Spawn the MCP server using npx
            this.process = spawn('npx', ['-y', 'terminator-mcp-agent@latest'], {
                stdio: ['pipe', 'pipe', 'pipe'],
                shell: true
            });

            this.process.stdout.on('data', (data) => {
                this.handleOutput(data);
            });

            this.process.stderr.on('data', (data) => {
                console.error('MCP Server stderr:', data.toString());
            });

            this.process.on('error', (error) => {
                console.error('Failed to start MCP server:', error);
                reject(error);
            });

            this.process.on('exit', (code) => {
                console.log(`MCP server exited with code ${code}`);
            });

            // Initialize the MCP connection
            this.initialize()
                .then(() => {
                    console.log('Terminator MCP server initialized successfully');
                    resolve();
                })
                .catch(reject);
        });
    }

    /**
     * Handle output from the MCP server
     */
    handleOutput(data) {
        this.buffer += data.toString();

        // Try to parse complete JSON-RPC messages
        const lines = this.buffer.split('\n');
        this.buffer = lines.pop() || ''; // Keep incomplete line in buffer

        for (const line of lines) {
            if (line.trim()) {
                try {
                    const message = JSON.parse(line);
                    this.handleMessage(message);
                } catch (error) {
                    console.error('Failed to parse MCP message:', line, error);
                }
            }
        }
    }

    /**
     * Handle a parsed MCP message
     */
    handleMessage(message) {
        if (message.id && this.pendingRequests.has(message.id)) {
            const { resolve, reject } = this.pendingRequests.get(message.id);
            this.pendingRequests.delete(message.id);

            if (message.error) {
                reject(new Error(message.error.message || JSON.stringify(message.error)));
            } else {
                resolve(message.result);
            }
        } else {
            // Handle notifications or other messages
            this.emit('message', message);
        }
    }

    /**
     * Send a JSON-RPC request to the MCP server
     */
    async sendRequest(method, params = {}) {
        return new Promise((resolve, reject) => {
            const id = ++this.messageId;
            const request = {
                jsonrpc: '2.0',
                id,
                method,
                params
            };

            this.pendingRequests.set(id, { resolve, reject });

            // Send the request
            const requestStr = JSON.stringify(request) + '\n';
            this.process.stdin.write(requestStr);

            // Set timeout
            setTimeout(() => {
                if (this.pendingRequests.has(id)) {
                    this.pendingRequests.delete(id);
                    reject(new Error(`Request timeout: ${method}`));
                }
            }, 60000); // 60 second timeout
        });
    }

    /**
     * Initialize the MCP connection
     */
    async initialize() {
        const result = await this.sendRequest('initialize', {
            protocolVersion: '2024-11-05',
            capabilities: {},
            clientInfo: {
                name: 'gemini-terminator-demo',
                version: '1.0.0'
            }
        });

        // Send initialized notification (required by MCP protocol)
        await this.sendNotification('notifications/initialized');

        this.initialized = true;
        return result;
    }

    /**
     * Send a notification (no response expected)
     */
    async sendNotification(method, params = {}) {
        const notification = {
            jsonrpc: '2.0',
            method,
            params
        };

        const notificationStr = JSON.stringify(notification) + '\n';
        this.process.stdin.write(notificationStr);
    }

    /**
     * Call a tool on the MCP server
     */
    async callTool(toolName, args = {}) {
        if (!this.initialized) {
            throw new Error('MCP client not initialized');
        }

        const result = await this.sendRequest('tools/call', {
            name: toolName,
            arguments: args
        });

        return result;
    }

    /**
     * Open an application
     */
    async openApplication(path) {
        return this.callTool('open_application', { path });
    }

    /**
     * Click an element using selector
     */
    async clickElement(selector, options = {}) {
        return this.callTool('click_element', {
            selector,
            ...options
        });
    }

    /**
     * Type text into an element
     */
    async typeIntoElement(selector, text, options = {}) {
        return this.callTool('type_into_element', {
            selector,
            text_to_type: text,
            ...options
        });
    }

    /**
     * Wait for an element to exist
     */
    async waitForElement(selector, timeoutMs = 5000) {
        return this.callTool('wait_for_element', {
            selector,
            condition: 'exists',
            timeout_ms: timeoutMs
        });
    }

    /**
     * Get the accessibility tree for a window
     */
    async getWindowTree(pid = null, options = {}) {
        const params = { ...options };
        if (pid !== null) {
            params.pid = pid;
        }
        return this.callTool('get_window_tree', params);
    }

    /**
     * Get list of running applications
     */
    async getApplications() {
        return this.callTool('get_applications_and_windows_list', {
            include_tree_after_action: false
        });
    }

    /**
     * Close the MCP connection
     */
    async close() {
        if (this.process) {
            this.process.kill();
            this.process = null;
        }
    }
}

// Test mode
if (process.argv.includes('--test')) {
    console.log('Testing Terminator MCP Client...');

    const client = new TerminatorMCPClient();

    client.start()
        .then(async () => {
            console.log('\n✓ MCP server started successfully');

            // Test: List available tools
            console.log('\nListing available tools...');
            const toolsList = await client.sendRequest('tools/list', {});
            console.log('✓ Available tools:', toolsList.tools?.map(t => t.name).join(', '));

            console.log('\n✓ All tests passed!');
            await client.close();
            process.exit(0);
        })
        .catch((error) => {
            console.error('\n✗ Test failed:', error);
            process.exit(1);
        });
}
