import { z } from 'zod';

// Re-export types from terminator.js
export type { Desktop, Locator, Element } from 'terminator.js';

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
 */
export interface WorkflowContext {
  /** Mutable data storage shared between steps */
  data: any;
  /** Additional state storage */
  state: Record<string, any>;
  /** Workflow input variables */
  variables: any;
}

/**
 * Step execution context
 */
export interface StepContext<TInput = any> {
  /** Desktop automation instance */
  desktop: import('terminator.js').Desktop;
  /** Workflow input (validated by Zod schema) */
  input: TInput;
  /** Shared workflow context */
  context: WorkflowContext;
  /** Logger instance */
  logger: Logger;
}

/**
 * Error recovery context
 */
export interface ErrorContext<TInput = any, TOutput = any> {
  /** The error that occurred */
  error: Error;
  /** Desktop instance for recovery actions */
  desktop: import('terminator.js').Desktop;
  /** Retry the step execution */
  retry: () => Promise<TOutput>;
  /** Current attempt number (0-indexed) */
  attempt: number;
  /** Workflow input */
  input: TInput;
  /** Shared context */
  context: WorkflowContext;
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
 * Step configuration
 */
export interface StepConfig<TInput = any, TOutput = any> {
  /** Unique step identifier */
  id: string;
  /** Human-readable step name */
  name: string;
  /** Optional step description */
  description?: string;

  /** Main step execution function */
  execute: (context: StepContext<TInput>) => Promise<TOutput | void>;

  /** Error recovery function */
  onError?: (
    context: ErrorContext<TInput, TOutput>
  ) => Promise<ErrorRecoveryResult | void>;

  /** Step timeout in milliseconds */
  timeout?: number;

  /** Condition to determine if step should run */
  condition?: (context: { input: TInput; context: WorkflowContext }) => boolean;
}

/**
 * Step instance
 */
export interface Step<TInput = any, TOutput = any> {
  config: StepConfig<TInput, TOutput>;

  /** Execute the step */
  run(context: StepContext<TInput>): Promise<TOutput | void>;

  /** Get step metadata */
  getMetadata(): {
    id: string;
    name: string;
    description?: string;
  };
}

/**
 * Workflow configuration
 */
export interface WorkflowConfig<TInput = any> {
  /** Workflow name */
  name: string;
  /** Optional workflow description */
  description?: string;
  /** Optional workflow version */
  version?: string;
  /** Input schema (Zod) */
  input: z.ZodSchema<TInput>;
  /** Optional tags */
  tags?: string[];
}

/**
 * Workflow execution context
 */
export interface WorkflowExecutionContext<TInput = any> {
  /** Current step being executed */
  step: Step;
  /** Workflow input */
  input: TInput;
  /** Shared context */
  context: WorkflowContext;
  /** Logger */
  logger: Logger;
}

/**
 * Workflow success handler context
 */
export interface WorkflowSuccessContext<TInput = any> {
  /** Workflow input */
  input: TInput;
  /** Final context state */
  context: WorkflowContext;
  /** Logger */
  logger: Logger;
  /** Execution duration in ms */
  duration: number;
}

/**
 * Workflow error handler context
 */
export interface WorkflowErrorContext<TInput = any> {
  /** The error that occurred */
  error: Error;
  /** Step where error occurred */
  step: Step;
  /** Workflow input */
  input: TInput;
  /** Context at time of error */
  context: WorkflowContext;
  /** Logger */
  logger: Logger;
}

/**
 * Workflow instance
 */
export interface Workflow<TInput = any> {
  config: WorkflowConfig<TInput>;
  steps: Step[];

  /** Run the workflow */
  run(
    input: TInput,
    desktop?: import('terminator.js').Desktop,
    logger?: Logger
  ): Promise<void>;

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
