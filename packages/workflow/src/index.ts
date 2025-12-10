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
    RetryMarker,
    success,
    WorkflowSuccessMarker,
    next,
    NextStepMarker,
} from "./types";

// Event streaming API
export { emit, createStepEmitter } from "./events";
export type {
    WorkflowEvent,
    ProgressEvent,
    StepEvent,
    ScreenshotEvent,
    DataEvent,
    LogEvent,
    StepEmitter,
} from "./events";

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
    SuccessResult,
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
