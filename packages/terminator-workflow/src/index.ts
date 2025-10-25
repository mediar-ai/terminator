/**
 * @mediar/terminator-workflow
 *
 * TypeScript SDK for building Terminator workflows with type safety,
 * error recovery, and easy parsing for mediar-app UI.
 */

export { createStep } from './step';
export { createWorkflow } from './workflow';

export type {
  Desktop,
  Locator,
  Logger,
  WorkflowContext,
  StepContext,
  ErrorContext,
  ErrorRecoveryResult,
  StepConfig,
  Step,
  WorkflowConfig,
  Workflow,
  WorkflowExecutionContext,
  WorkflowSuccessContext,
  WorkflowErrorContext,
} from './types';

export { ConsoleLogger } from './types';

// Re-export zod for convenience
export { z } from 'zod';
