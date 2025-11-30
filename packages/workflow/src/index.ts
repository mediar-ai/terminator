/**
 * @mediar-ai/workflow
 *
 * TypeScript SDK for building Terminator workflows with type safety,
 * error recovery, and easy parsing for mediar-app UI.
 */

export { createStep } from "./step";
export { createWorkflow, WorkflowBuilder } from "./workflow";
export { createWorkflowRunner, WorkflowRunner } from "./runner";
export {
    WorkflowError,
    createWorkflowError,
    retry,
    RetrySignal,
} from "./types";

export type {
    Desktop,
    Locator,
    Logger,
    WorkflowContext,
    StepContext,
    ErrorContext,
    ErrorRecoveryResult,
    ExpectationResult,
    ExpectationContext,
    ExecutionStatus,
    ErrorCategory,
    ExecuteError,
    ExecutionResponse,
    StepResult,
    StepConfig,
    Step,
    WorkflowConfig,
    Workflow,
    WorkflowExecutionContext,
    WorkflowSuccessContext,
    WorkflowErrorContext,
} from "./types";

export type { WorkflowRunnerOptions, WorkflowState } from "./runner";

export { ConsoleLogger } from "./types";

// Re-export zod for convenience
export { z } from "zod";
