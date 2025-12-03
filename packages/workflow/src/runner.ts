import { Workflow, WorkflowContext, Logger, ConsoleLogger, isWorkflowSuccess } from './types';
import { Desktop } from '@mediar-ai/terminator';

export interface WorkflowRunnerOptions {
  workflow: Workflow;
  inputs: any;
  startFromStep?: string;
  endAtStep?: string;
  restoredState?: WorkflowState;
}

export interface WorkflowState {
  context: WorkflowContext;
  stepResults: Record<string, { status: string; result?: any; error?: string }>;
  lastStepId?: string;
  lastStepIndex: number;
}

export class WorkflowRunner {
  private workflow: Workflow;
  private inputs: any;
  private startFromStep?: string;
  private endAtStep?: string;
  private state: WorkflowState;
  private desktop: Desktop;
  private logger: Logger;

  constructor(options: WorkflowRunnerOptions) {
    this.workflow = options.workflow;
    this.inputs = options.inputs;
    this.startFromStep = options.startFromStep;
    this.endAtStep = options.endAtStep;
    this.logger = new ConsoleLogger();

    // Initialize or restore state
    if (options.restoredState) {
      this.state = {
        ...options.restoredState,
        stepResults: options.restoredState.stepResults || {},
      };
      this.logger.info('🔄 Restored state from previous run');
    } else {
      this.state = {
        context: {
          data: {},
          state: {},
          variables: this.inputs,
        },
        stepResults: {},
        lastStepIndex: -1,
      };
    }

    this.desktop = new Desktop();
  }

  async run(): Promise<{ status: string; lastStepId?: string; lastStepIndex: number; error?: string }> {
    const steps = this.workflow.steps;

    // Find start and end indices
    let startIndex = 0;
    if (this.startFromStep) {
      startIndex = steps.findIndex(s => s.config.id === this.startFromStep);
      if (startIndex === -1) {
        const error = `Start step '${this.startFromStep}' not found`;
        this.logger.error(error);
        throw new Error(error);
      }
      this.logger.info(`📍 Starting from step: ${this.startFromStep} (index ${startIndex})`);
    }

    let endIndex = steps.length - 1;
    if (this.endAtStep) {
      endIndex = steps.findIndex(s => s.config.id === this.endAtStep);
      if (endIndex === -1) {
        const error = `End step '${this.endAtStep}' not found`;
        this.logger.error(error);
        throw new Error(error);
      }
      this.logger.info(`🎯 Stopping at step: ${this.endAtStep} (index ${endIndex})`);
    }

    // Execute steps
    for (let i = startIndex; i <= endIndex; i++) {
      const step = steps[i];

      this.logger.info(`\n[${i + 1}/${steps.length}] ${step.config.name}`);

      try {
        // Check if step has condition
        if (step.config.condition) {
          const shouldRun = step.config.condition({
            input: this.inputs,
            context: this.state.context,
          });

          if (!shouldRun) {
            this.logger.info('⏭️  Skipping step (condition not met)');
            this.state.stepResults[step.config.id] = {
              status: 'skipped',
            };
            continue;
          }
        }

        // Execute step
        const result = await step.run({
          desktop: this.desktop,
          input: this.inputs,
          context: this.state.context,
          logger: this.logger,
        });

        // Check for early success return
        if (isWorkflowSuccess(result)) {
          this.logger.success(`✅ Workflow completed early`);
          this.state.context.data = result.result;
          this.state.lastStepId = step.config.id;
          this.state.lastStepIndex = i;
          return {
            status: 'success',
            lastStepId: this.state.lastStepId,
            lastStepIndex: this.state.lastStepIndex,
          };
        }

        // Save step result
        this.state.stepResults[step.config.id] = {
          status: 'success',
          result,
        };
        this.state.lastStepId = step.config.id;
        this.state.lastStepIndex = i;

      } catch (error: any) {
        this.logger.error(`❌ Step failed: ${error.message}`);

        // Save step error
        this.state.stepResults[step.config.id] = {
          status: 'error',
          error: error.message,
        };
        this.state.lastStepId = step.config.id;
        this.state.lastStepIndex = i;

        // Return error result
        return {
          status: 'error',
          lastStepId: this.state.lastStepId,
          lastStepIndex: this.state.lastStepIndex,
          error: error.message,
        };
      }
    }

    return {
      status: 'success',
      lastStepId: this.state.lastStepId,
      lastStepIndex: this.state.lastStepIndex,
    };
  }

  getState(): WorkflowState {
    return this.state;
  }
}

export function createWorkflowRunner(options: WorkflowRunnerOptions): WorkflowRunner {
  return new WorkflowRunner(options);
}
