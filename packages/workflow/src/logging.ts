/**
 * Workflow Logging System
 *
 * Provides structured logging via Windows named pipes for clean IPC with the
 * MCP agent. Falls back to stderr with level prefixes for backward compatibility.
 *
 * @example
 * ```typescript
 * import { log, setupConsoleRedirect } from '@mediar-ai/workflow';
 *
 * // Setup console redirect at workflow start (optional but recommended)
 * setupConsoleRedirect();
 *
 * // Use structured logging
 * log.info('Processing started');
 * log.debug('Item details', { id: 123, name: 'test' });
 * log.warn('Rate limit approaching');
 * log.error('Failed to connect', { error: 'timeout' });
 * ```
 */

import * as fs from 'fs';

/**
 * Log entry structure sent to the MCP agent
 */
interface LogEntry {
    level: 'debug' | 'info' | 'warn' | 'error';
    message: string;
    data?: any;
    timestamp: string;
}

/**
 * Log transport - handles sending logs to the MCP agent via named pipe
 */
class LogTransport {
    private pipeStream: fs.WriteStream | null = null;
    private pipePath: string | null = null;
    private connectionAttempted = false;
    private useStderr = false;

    constructor() {
        // Get pipe path from environment
        this.pipePath = process.env.MCP_LOG_PIPE || null;

        // If no pipe path, fall back to stderr with level prefixes
        if (!this.pipePath) {
            this.useStderr = true;
        }
    }

    /**
     * Connect to the named pipe (lazy connection on first write)
     */
    private connect(): boolean {
        if (this.useStderr) {
            return true;
        }

        if (this.pipeStream) {
            return true;
        }

        if (this.connectionAttempted) {
            return false;
        }

        this.connectionAttempted = true;

        try {
            this.pipeStream = fs.createWriteStream(this.pipePath!, { flags: 'w' });

            this.pipeStream.on('error', (err) => {
                // Silently fall back to stderr - don't log the error to avoid recursion
                this.pipeStream = null;
                this.useStderr = true;
            });

            return true;
        } catch (err: any) {
            this.useStderr = true;
            return false;
        }
    }

    /**
     * Send a log entry
     */
    send(level: LogEntry['level'], message: string, data?: any): void {
        if (this.useStderr || !this.connect()) {
            // Fall back to stderr with level prefix (for backward compatibility)
            const prefix = `[${level.toUpperCase()}]`;
            const dataStr = data ? ` ${JSON.stringify(data)}` : '';
            process.stderr.write(`${prefix} ${message}${dataStr}\n`);
            return;
        }

        const entry: LogEntry = {
            level,
            message,
            data,
            timestamp: new Date().toISOString(),
        };

        const json = JSON.stringify(entry) + '\n';

        try {
            this.pipeStream!.write(json);
        } catch (err: any) {
            // Fall back to stderr on write error
            const prefix = `[${level.toUpperCase()}]`;
            process.stderr.write(`${prefix} ${message}\n`);
        }
    }

    /**
     * Close the transport
     */
    close(): void {
        if (this.pipeStream) {
            this.pipeStream.end();
            this.pipeStream = null;
        }
    }

    /**
     * Check if using pipe transport
     */
    isUsingPipe(): boolean {
        return !this.useStderr && this.pipePath !== null;
    }
}

// Global transport instance
const transport = new LogTransport();

// Cleanup on process exit
process.on('exit', () => transport.close());
process.on('SIGINT', () => { transport.close(); process.exit(0); });
process.on('SIGTERM', () => { transport.close(); process.exit(0); });

/**
 * Structured logger for workflow code
 *
 * @example
 * ```typescript
 * import { log } from '@mediar-ai/workflow';
 *
 * log.info('Starting workflow');
 * log.debug('Configuration', { timeout: 5000 });
 * log.warn('Retrying operation');
 * log.error('Failed', { code: 'TIMEOUT' });
 * ```
 */
export const log = {
    /**
     * Log a debug message (verbose, for troubleshooting)
     */
    debug(message: string, data?: any): void {
        transport.send('debug', message, data);
    },

    /**
     * Log an info message (general information)
     */
    info(message: string, data?: any): void {
        transport.send('info', message, data);
    },

    /**
     * Log a warning message (potential issues)
     */
    warn(message: string, data?: any): void {
        transport.send('warn', message, data);
    },

    /**
     * Log an error message (failures)
     */
    error(message: string, data?: any): void {
        transport.send('error', message, data);
    },
};

/**
 * Format arguments to string for console methods
 */
const formatArgs = (...args: any[]): string =>
    args.map(a => typeof a === 'object' ? JSON.stringify(a) : String(a)).join(' ');

/**
 * Setup console redirect to use the logging transport
 *
 * This redirects console.log, console.info, console.warn, console.error, and
 * console.debug to use the named pipe transport when available.
 *
 * Call this at the start of your workflow for clean logging.
 *
 * @example
 * ```typescript
 * import { setupConsoleRedirect } from '@mediar-ai/workflow';
 *
 * // At workflow start
 * setupConsoleRedirect();
 *
 * // Now console methods go through the pipe
 * console.log('This goes to the log pipe');
 * console.error('Errors too');
 * ```
 */
export function setupConsoleRedirect(): void {
    const originalLog = console.log;
    const originalError = console.error;

    // console.log - Only allow JSON output to stdout (for result parsing)
    console.log = (...args: any[]) => {
        if (args.length === 1 && typeof args[0] === 'string' && args[0].startsWith('{')) {
            originalLog(...args);
        } else {
            log.info(formatArgs(...args));
        }
    };

    console.info = (...args: any[]) => {
        log.info(formatArgs(...args));
    };

    console.warn = (...args: any[]) => {
        log.warn(formatArgs(...args));
    };

    console.error = (...args: any[]) => {
        log.error(formatArgs(...args));
    };

    console.debug = (...args: any[]) => {
        log.debug(formatArgs(...args));
    };
}

/**
 * For testing: get the current transport mode
 * @internal
 */
export function _getLogTransportMode(): 'pipe' | 'stderr' {
    return process.env.MCP_LOG_PIPE ? 'pipe' : 'stderr';
}
