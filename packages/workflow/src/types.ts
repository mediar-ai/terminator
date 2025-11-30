import { z } from "zod";

// Re-export types from terminator.js
export type { Desktop, Locator, Element } from "@mediar-ai/terminator";

/**
 * Logger interface
 */
export interface Logger {
    info(message: string): void;
    success(message: string): void;
    warn(message: string): void;
    error(message: string): void;
    debug(message: string): void;
}

/**
 * Workflow context shared between steps
 * @template TInput - Type of workflow input
 * @template TState - Type of accumulated state from previous steps
 */
export interface WorkflowContext<TInput = any, TState = Record<string, any>> {
    /** Mutable data storage shared between steps - keyed by step ID */
    data: Record<string, any>;
    /** Additional state storage - typed based on accumulated step outputs */
    state: TState;
    /** Workflow input variables - typed from Zod schema */
    variables: TInput;
}

/**
 * Step execution context
 * @template TInput - Type of workflow input
 * @template TState - Type of accumulated state from previous steps
 */
export interface StepContext<TInput = any, TState = Record<string, any>> {
    /** Desktop automation instance */
    desktop: import("@mediar-ai/terminator").Desktop;
    /** Workflow input (validated by Zod schema) */
    input: TInput;
    /** Shared workflow context with typed state and variables */
    context: WorkflowContext<TInput, TState>;
    /** Logger instance */
    logger: Logger;
}

/**
 * Error recovery context for handling step failures
 *
 * ## Retry Model
 *
 * There are two levels of retry in Mediar workflows:
 *
 * 1. **Infrastructure Retry** (automatic, handled by executor):
 *    - MCP server unreachable, VM down, network timeouts
 *    - Handled automatically by the Rust executor with exponential backoff
 *    - You don't need to handle these in your workflow code
 *
 * 2. **Business Logic Retry** (manual, handled in `onError`):
 *    - Application errors: element not found, validation failed, unexpected state
 *    - Use `retry()` to re-execute the step after recovery actions
 *    - You control the retry logic, max attempts, and recovery strategy
 *
 * @template TInput - Type of workflow input
 * @template TOutput - Type of step output
 * @template TState - Type of accumulated state from previous steps
 *
 * @example
 * ```typescript
 * createStep({
 *   id: 'submit_form',
 *   name: 'Submit Form',
 *   execute: async ({ desktop }) => {
 *     await desktop.locator('role:button[name="Submit"]').click();
 *   },
 *   onError: async ({ error, retry, desktop, logger }) => {
 *     if (error.message.includes('Session expired')) {
 *       logger.info('Session expired, re-authenticating...');
 *       await desktop.locator('role:button[name="Login"]').click();
 *       return retry();
 *     }
 *     return { recoverable: false, reason: error.message };
 *   }
 * })
 * ```
 */
export interface ErrorContext<
    TInput = any,
    TOutput = any,
    TState = Record<string, any>,
> {
    /** The error that occurred */
    error: Error;
    /** Desktop instance for recovery actions */
    desktop: import("@mediar-ai/terminator").Desktop;
    /**
     * Retry the step execution after performing recovery actions.
     * Call this to re-execute the step's `execute()` function.
     */
    retry: () => Promise<TOutput>;
    /** Current attempt number (0-indexed) */
    attempt: number;
    /** Workflow input */
    input: TInput;
    /** Shared context with typed state and variables */
    context: WorkflowContext<TInput, TState>;
    /** Logger instance */
    logger: Logger;
}

/**
 * Error recovery result
 */
export interface ErrorRecoveryResult {
    /** Whether the error can be recovered from */
    recoverable: boolean;
    /** Reason for the recovery decision */
    reason?: string;
}

/**
 * Expectation validation result
 */
export interface ExpectationResult {
    /** Whether the expectation was met */
    success: boolean;
    /** Optional message describing the result */
    message?: string;
    /** Optional custom data */
    data?: any;
}

/**
 * Expectation context - runs after execute() to verify step outcome
 * @template TInput - Type of workflow input
 * @template TOutput - Type of step output
 * @template TState - Type of accumulated state from previous steps
 */
export interface ExpectationContext<
    TInput = any,
    TOutput = any,
    TState = Record<string, any>,
> {
    /** Desktop instance for validation checks */
    desktop: import("@mediar-ai/terminator").Desktop;
    /** Workflow input */
    input: TInput;
    /** Result from execute() */
    result: TOutput;
    /** Shared context with typed state and variables */
    context: WorkflowContext<TInput, TState>;
    /** Logger instance */
    logger: Logger;
}

/**
 * Workflow execution status
 */
export type ExecutionStatus =
    | "success"
    | "error"
    | "warning"
    | "user_input_required";

/**
 * Error category
 */
export type ErrorCategory = "business" | "technical";

/**
 * Execute error information
 */
export interface ExecuteError {
    category: ErrorCategory;
    code: string;
    message: string;
    recoverable?: boolean;
    metadata?: Record<string, any>;
}

/**
 * Creates a structured workflow error
 *
 * @example
 * ```typescript
 * throw WorkflowError({
 *   category: 'business',
 *   code: 'SAP_DUPLICATE_INVOICE',
 *   message: 'Invoice already exists in SAP',
 *   recoverable: true,
 *   metadata: { invoiceNumber: '12345' }
 * });
 * ```
 */
export function WorkflowError(error: ExecuteError): Error & ExecuteError {
    const err = new Error(error.message) as Error & ExecuteError;
    err.category = error.category;
    err.code = error.code;
    err.recoverable = error.recoverable;
    err.metadata = error.metadata;
    return err;
}

/**
 * @deprecated Use WorkflowError instead (follows Error/TypeError naming convention)
 */
export const createWorkflowError = WorkflowError;

/**
 * Workflow execution response
 */
export interface ExecutionResponse<TData = any> {
    /** Well-rendered status in UI */
    status: ExecutionStatus;
    /** Error information (if status is 'error') */
    error?: ExecuteError;
    /** Optional custom data (less well-rendered in UI) */
    data?: TData;
    /** Optional user-facing message */
    message?: string;
    /** Last completed step ID (for state persistence) */
    lastStepId?: string;
    /** Last completed step index (for state persistence) */
    lastStepIndex?: number;
    /** Workflow state (for resumption) */
    state?: any;
}

/**
 * Step execution result - enforces structured output
 * @template TData - Type of data returned by the step
 * @template TStateUpdate - Type of state updates (will be merged with existing state)
 */
export interface StepResult<TData = any, TStateUpdate = Record<string, any>> {
    /** Optional data to store in workflow context */
    data?: TData;
    /** Optional state updates to merge into workflow context.state */
    state?: TStateUpdate;
}

/**
 * Step configuration
 * @template TInput - Type of workflow input
 * @template TOutput - Type of step output (data)
 * @template TStateIn - Type of state available from previous steps
 * @template TStateOut - Type of state updates this step produces
 */
export interface StepConfig<
    TInput = any,
    TOutput = any,
    TStateIn extends Record<string, any> = Record<string, any>,
    TStateOut extends Record<string, any> = Record<string, any>,
> {
    /** Unique step identifier */
    id: string;
    /** Human-readable step name */
    name: string;
    /** Optional step description */
    description?: string;

    /**
     * Main step execution function
     *
     * Should return either:
     * - StepResult with structured data/state updates
     * - void (for side-effect only steps)
     * - Plain object (backward compatibility - will be wrapped in StepResult)
     */
    execute: (
        context: StepContext<TInput, TStateIn>,
    ) => Promise<StepResult<TOutput, TStateOut> | TOutput | void>;

    /** Expectation validation - runs after execute() to verify outcome */
    expect?: (
        context: ExpectationContext<TInput, TOutput, TStateIn>,
    ) => Promise<ExpectationResult>;

    /**
     * Error recovery handler for business logic failures.
     *
     * Use this to handle application-level errors like:
     * - Element not found / UI state issues
     * - Validation failures
     * - Business rule violations
     * - Session timeouts
     *
     * **Note:** Infrastructure errors (MCP unreachable, VM down, network issues)
     * are automatically retried by the executor - you don't need to handle them here.
     *
     * @example
     * ```typescript
     * onError: async ({ error, retry, attempt, logger }) => {
     *   if (attempt >= 3) {
     *     return { recoverable: false, reason: 'Max retries exceeded' };
     *   }
     *   if (error.message.includes('Element not found')) {
     *     logger.info(`Retry attempt ${attempt + 1}/3`);
     *     await new Promise(r => setTimeout(r, 1000));
     *     return retry();
     *   }
     *   return { recoverable: false, reason: error.message };
     * }
     * ```
     */
    onError?: (
        context: ErrorContext<TInput, TOutput, TStateIn>,
    ) => Promise<ErrorRecoveryResult | void>;

    /** Step timeout in milliseconds */
    timeout?: number;

    /**
     * Number of automatic retries on failure (sugar for onError + retry pattern).
     *
     * When set, automatically retries the step up to this many times with
     * exponential backoff. Cannot be used together with `onError`.
     *
     * **Note:** This is for business logic retries only. Infrastructure errors
     * are handled automatically by the executor.
     *
     * @default undefined (no automatic retries)
     *
     * @example
     * ```typescript
     * createStep({
     *   id: 'flaky_step',
     *   name: 'Flaky Operation',
     *   retries: 3,
     *   retryDelayMs: 1000,
     *   execute: async ({ desktop }) => {
     *     await desktop.locator('role:button').click();
     *   }
     * })
     * ```
     */
    retries?: number;

    /**
     * Initial delay in milliseconds between retries (default: 1000ms).
     * Uses exponential backoff: delay doubles after each retry.
     * Only used when `retries` is set.
     */
    retryDelayMs?: number;

    /** Condition to determine if step should run */
    condition?: (context: {
        input: TInput;
        context: WorkflowContext<TInput, TStateIn>;
    }) => boolean;

    /**
     * Next step to execute after this one.
     * Can be a step ID string or a function that returns a step ID.
     * If not provided, execution continues to the next step in sequence.
     * Use this for branching, loops, or conditional flow control.
     *
     * @example
     * // Static jump
     * next: 'step_id'
     *
     * // Conditional jump
     * next: ({ context }) => context.state.isDuplicate ? 'handle_dupe' : 'process'
     */
    next?:
        | string
        | ((context: {
              input: TInput;
              context: WorkflowContext<TInput, TStateIn>;
          }) => string | undefined);
}

/**
 * Step instance
 * @template TInput - Type of workflow input
 * @template TOutput - Type of step output
 * @template TStateIn - Type of state available from previous steps
 * @template TStateOut - Type of state updates this step produces
 */
export interface Step<
    TInput = any,
    TOutput = any,
    TStateIn extends Record<string, any> = Record<string, any>,
    TStateOut extends Record<string, any> = Record<string, any>,
> {
    config: StepConfig<TInput, TOutput, TStateIn, TStateOut>;

    /** Execute the step */
    run(context: StepContext<TInput, TStateIn>): Promise<TOutput | void>;

    /** Get step metadata */
    getMetadata(): {
        id: string;
        name: string;
        description?: string;
    };
}

/**
 * Workflow configuration (user-facing)
 *
 * Note: name, version, and description are automatically read from package.json.
 * Do NOT pass these fields - they will be ignored.
 */
export interface WorkflowConfig<TInput = any> {
    /** Input schema (Zod) */
    input: z.ZodSchema<TInput>;
    /** Optional tags */
    tags?: string[];
    /** Steps to execute in sequence */
    steps?: Step[];
    /** Workflow-level error handler */
    onError?: (
        context: WorkflowErrorContext<TInput>,
    ) => Promise<ExecutionResponse | void>;
}

/**
 * Internal resolved workflow configuration with metadata from package.json
 * @internal
 */
export interface ResolvedWorkflowConfig<
    TInput = any,
> extends WorkflowConfig<TInput> {
    /** Workflow name (from package.json) */
    name: string;
    /** Workflow description (from package.json) */
    description?: string;
    /** Workflow version (from package.json) */
    version?: string;
}

/**
 * Workflow execution context
 */
export interface WorkflowExecutionContext<
    TInput = any,
    TState = Record<string, any>,
> {
    /** Current step being executed */
    step: Step;
    /** Workflow input */
    input: TInput;
    /** Shared context with typed state and variables */
    context: WorkflowContext<TInput, TState>;
    /** Logger */
    logger: Logger;
}

/**
 * Workflow success handler context
 */
export interface WorkflowSuccessContext<
    TInput = any,
    TState = Record<string, any>,
> {
    /** Workflow input */
    input: TInput;
    /** Final context state with typed state and variables */
    context: WorkflowContext<TInput, TState>;
    /** Logger */
    logger: Logger;
    /** Execution duration in ms */
    duration: number;
}

/**
 * Workflow error handler context
 */
export interface WorkflowErrorContext<
    TInput = any,
    TState = Record<string, any>,
> {
    /** The error that occurred */
    error: Error;
    /** Desktop instance for recovery actions */
    desktop: import("@mediar-ai/terminator").Desktop;
    /** Step where error occurred */
    step: Step;
    /** Workflow input */
    input: TInput;
    /** Context at time of error with typed state and variables */
    context: WorkflowContext<TInput, TState>;
    /** Logger */
    logger: Logger;
}

/**
 * Workflow instance
 */
export interface Workflow<TInput = any> {
    config: ResolvedWorkflowConfig<TInput>;
    steps: Step[];

    /** Run the workflow */
    run(
        input: TInput,
        desktop?: import("@mediar-ai/terminator").Desktop,
        logger?: Logger,
    ): Promise<ExecutionResponse>;

    /** Get workflow metadata */
    getMetadata(): {
        name: string;
        description?: string;
        version?: string;
        input: z.ZodSchema<TInput>;
        steps: Array<{
            id: string;
            name: string;
            description?: string;
        }>;
    };
}

/**
 * Console logger implementation
 */
export class ConsoleLogger implements Logger {
    info(message: string): void {
        console.log(message);
    }

    success(message: string): void {
        console.log(message);
    }

    warn(message: string): void {
        console.warn(message);
    }

    error(message: string): void {
        console.error(message);
    }

    debug(message: string): void {
        console.debug(message);
    }
}

/**
 * Signal class for triggering step retry from anywhere in execute()
 * @internal
 */
export class RetrySignal extends Error {
    readonly _isRetrySignal = true;

    constructor() {
        super("Retry signal");
        this.name = "RetrySignal";
    }
}

/**
 * Trigger a step retry from within execute().
 * Throw this to re-execute the current step.
 *
 * @example
 * ```typescript
 * import { createStep, retry } from '@mediar-ai/workflow';
 *
 * createStep({
 *   id: 'click_button',
 *   name: 'Click Button',
 *   execute: async ({ desktop }) => {
 *     const button = await desktop.locator('role:button').first();
 *     if (!button) {
 *       throw retry(); // Re-execute this step
 *     }
 *     await button.click();
 *   }
 * });
 * ```
 */
export function retry(): RetrySignal {
    return new RetrySignal();
}
