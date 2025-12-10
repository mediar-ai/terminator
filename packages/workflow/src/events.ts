/**
 * Workflow Events System
 *
 * Enables TypeScript workflows to emit real-time events that are:
 * 1. Captured by the MCP agent's stderr parser
 * 2. Forwarded as MCP notifications to connected clients
 * 3. Included in the final workflow result for post-execution analysis
 *
 * @example
 * ```typescript
 * import { createStep, emit } from '@mediar-ai/workflow';
 *
 * createStep({
 *   id: 'login',
 *   name: 'Login',
 *   execute: async ({ desktop }) => {
 *     emit.progress(1, 3, 'Starting login...');
 *     await desktop.locator('role:textbox').fill('user@example.com');
 *
 *     emit.progress(2, 3, 'Entering credentials...');
 *     await desktop.locator('role:button[name="Submit"]').click();
 *
 *     emit.progress(3, 3, 'Login complete');
 *     emit.screenshot('/tmp/login-success.png', 'Login confirmation');
 *   }
 * });
 * ```
 */

// Event type discriminator for Rust-side parsing
const EVENT_PREFIX = '__mcp_event__';

/**
 * Base event structure - all events extend this
 */
interface BaseEvent {
    __mcp_event__: true;
    type: string;
    timestamp: string;
}

/**
 * Progress event - maps to MCP notifications/progress
 */
export interface ProgressEvent extends BaseEvent {
    type: 'progress';
    current: number;
    total?: number;
    message?: string;
}

/**
 * Step lifecycle events
 */
export interface StepEvent extends BaseEvent {
    type: 'step_started' | 'step_completed' | 'step_failed';
    stepId: string;
    stepName: string;
    stepIndex?: number;
    totalSteps?: number;
    duration?: number;
    error?: string;
}

/**
 * Screenshot event - for visual debugging
 */
export interface ScreenshotEvent extends BaseEvent {
    type: 'screenshot';
    path?: string;
    base64?: string;
    annotation?: string;
    element?: string;
}

/**
 * Custom data event - for arbitrary workflow data
 */
export interface DataEvent extends BaseEvent {
    type: 'data';
    key: string;
    value: any;
}

/**
 * Log event - structured logging with levels
 */
export interface LogEvent extends BaseEvent {
    type: 'log';
    level: 'debug' | 'info' | 'warn' | 'error';
    message: string;
    data?: any;
}

/**
 * Union of all event types
 */
export type WorkflowEvent =
    | ProgressEvent
    | StepEvent
    | ScreenshotEvent
    | DataEvent
    | LogEvent;

/**
 * Event payload without the automatically added fields
 */
type EventPayload = {
    type: string;
    [key: string]: any;
};

/**
 * Internal: Emit an event to stderr for MCP agent to capture
 */
function emitEvent(event: EventPayload): void {
    const fullEvent = {
        __mcp_event__: true as const,
        timestamp: new Date().toISOString(),
        ...event,
    };
    // Write to stderr as JSON - the Rust side parses lines starting with {"__mcp_event__":true
    console.error(JSON.stringify(fullEvent));
}

/**
 * Event emitter with typed methods for great DX
 *
 * @example
 * ```typescript
 * import { emit } from '@mediar-ai/workflow';
 *
 * // Progress updates (shown in MCP client progress bar)
 * emit.progress(1, 5, 'Starting...');
 * emit.progress(2, 5, 'Processing items...');
 *
 * // Structured logging
 * emit.log('info', 'Found 10 items to process');
 * emit.log('debug', 'Item details', { items: [...] });
 *
 * // Screenshots for debugging
 * emit.screenshot('/tmp/before.png', 'State before action');
 *
 * // Custom data
 * emit.data('orderTotal', 125.50);
 * emit.data('extractedEmails', ['a@b.com', 'c@d.com']);
 * ```
 */
export const emit = {
    /**
     * Emit a progress update
     * @param current - Current progress value (e.g., step number)
     * @param total - Total steps (optional, omit for indeterminate progress)
     * @param message - Human-readable progress message
     */
    progress(current: number, total?: number, message?: string): void {
        emitEvent({ type: 'progress', current, total, message });
    },

    /**
     * Emit a step started event (usually called automatically by the runner)
     */
    stepStarted(stepId: string, stepName: string, stepIndex?: number, totalSteps?: number): void {
        emitEvent({ type: 'step_started', stepId, stepName, stepIndex, totalSteps });
    },

    /**
     * Emit a step completed event (usually called automatically by the runner)
     */
    stepCompleted(stepId: string, stepName: string, duration: number, stepIndex?: number, totalSteps?: number): void {
        emitEvent({ type: 'step_completed', stepId, stepName, duration, stepIndex, totalSteps });
    },

    /**
     * Emit a step failed event (usually called automatically by the runner)
     */
    stepFailed(stepId: string, stepName: string, error: string, duration: number): void {
        emitEvent({ type: 'step_failed', stepId, stepName, error, duration });
    },

    /**
     * Emit a screenshot event for visual debugging
     * @param pathOrBase64 - File path or base64-encoded image data
     * @param annotation - Description of what the screenshot shows
     * @param element - Optional element selector that was captured
     */
    screenshot(pathOrBase64: string, annotation?: string, element?: string): void {
        const isBase64 = pathOrBase64.startsWith('data:') || pathOrBase64.length > 500;
        emitEvent({
            type: 'screenshot',
            ...(isBase64 ? { base64: pathOrBase64 } : { path: pathOrBase64 }),
            annotation,
            element,
        });
    },

    /**
     * Emit custom data for post-processing or display
     * @param key - Data identifier
     * @param value - Any JSON-serializable value
     */
    data(key: string, value: any): void {
        emitEvent({ type: 'data', key, value });
    },

    /**
     * Emit a structured log message
     * @param level - Log severity level
     * @param message - Log message
     * @param data - Optional additional data
     */
    log(level: 'debug' | 'info' | 'warn' | 'error', message: string, data?: any): void {
        emitEvent({ type: 'log', level, message, data });
    },

    /**
     * Emit a raw event (for advanced use cases)
     */
    raw(event: Omit<WorkflowEvent, '__mcp_event__' | 'timestamp'>): void {
        emitEvent(event);
    },
};

/**
 * Create a scoped emitter for a specific step
 * Automatically includes step context in all events
 *
 * @example
 * ```typescript
 * createStep({
 *   id: 'process_items',
 *   name: 'Process Items',
 *   execute: async ({ desktop }) => {
 *     const stepEmit = createStepEmitter('process_items', 'Process Items', 2, 5);
 *
 *     stepEmit.progress(0, 10, 'Starting...');
 *     for (let i = 0; i < 10; i++) {
 *       await processItem(i);
 *       stepEmit.progress(i + 1, 10, `Processed item ${i + 1}`);
 *     }
 *   }
 * });
 * ```
 */
export function createStepEmitter(stepId: string, stepName: string, stepIndex?: number, totalSteps?: number) {
    return {
        progress(current: number, total?: number, message?: string): void {
            emit.progress(current, total, message ? `[${stepName}] ${message}` : `[${stepName}] Step ${current}/${total || '?'}`);
        },
        log(level: 'debug' | 'info' | 'warn' | 'error', message: string, data?: any): void {
            emit.log(level, `[${stepName}] ${message}`, data);
        },
        screenshot(pathOrBase64: string, annotation?: string, element?: string): void {
            emit.screenshot(pathOrBase64, annotation ? `[${stepName}] ${annotation}` : `[${stepName}] Screenshot`, element);
        },
        data(key: string, value: any): void {
            emit.data(`${stepId}.${key}`, value);
        },
        started(): void {
            emit.stepStarted(stepId, stepName, stepIndex, totalSteps);
        },
        completed(duration: number): void {
            emit.stepCompleted(stepId, stepName, duration, stepIndex, totalSteps);
        },
        failed(error: string, duration: number): void {
            emit.stepFailed(stepId, stepName, error, duration);
        },
    };
}

export type StepEmitter = ReturnType<typeof createStepEmitter>;
