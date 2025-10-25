import { z } from 'zod';
import { Desktop } from 'terminator.js';
import type {
  Workflow,
  WorkflowConfig,
  Step,
  WorkflowContext,
  WorkflowSuccessContext,
  WorkflowErrorContext,
  ConsoleLogger,
  Logger,
} from './types';

/**
 * Workflow builder for composing steps
 */
class WorkflowBuilder<TInput = any> {
  private config: WorkflowConfig<TInput>;
  private steps: Step[] = [];
  private successHandler?: (context: WorkflowSuccessContext<TInput>) => Promise<void>;
  private errorHandler?: (context: WorkflowErrorContext<TInput>) => Promise<void>;

  constructor(config: WorkflowConfig<TInput>) {
    this.config = config;
  }

  /**
   * Add a step to the workflow
   */
  step<TOutput = any>(step: Step<TInput, TOutput>): this {
    this.steps.push(step);
    return this;
  }

  /**
   * Set success handler
   */
  onSuccess(handler: (context: WorkflowSuccessContext<TInput>) => Promise<void>): this {
    this.successHandler = handler;
    return this;
  }

  /**
   * Set error handler
   */
  onError(handler: (context: WorkflowErrorContext<TInput>) => Promise<void>): this {
    this.errorHandler = handler;
    return this;
  }

  /**
   * Build the workflow
   */
  build(): Workflow<TInput> {
    return createWorkflowInstance(
      this.config,
      this.steps,
      this.successHandler,
      this.errorHandler
    );
  }
}

/**
 * Creates a workflow instance
 */
function createWorkflowInstance<TInput = any>(
  config: WorkflowConfig<TInput>,
  steps: Step[],
  successHandler?: (context: WorkflowSuccessContext<TInput>) => Promise<void>,
  errorHandler?: (context: WorkflowErrorContext<TInput>) => Promise<void>
): Workflow<TInput> {
  return {
    config,
    steps,

    async run(input: TInput, desktop?: Desktop, logger?: Logger): Promise<void> {
      const log = logger || new ConsoleLogger();
      const startTime = Date.now();

      log.info('='.repeat(60));
      log.info(`🚀 Starting workflow: ${config.name}`);
      if (config.description) {
        log.info(`   ${config.description}`);
      }
      log.info('='.repeat(60));
      log.info('');

      // Validate input
      const validationResult = config.input.safeParse(input);
      if (!validationResult.success) {
        log.error('❌ Input validation failed:');
        log.error(JSON.stringify(validationResult.error.format(), null, 2));
        throw new Error('Input validation failed');
      }

      const validatedInput = validationResult.data;

      // Initialize context
      const context: WorkflowContext = {
        data: {},
        state: {},
        variables: validatedInput,
      };

      // Get desktop instance (either passed or create new one)
      const desktopInstance = desktop || new Desktop();

      try {
        // Execute steps sequentially
        for (let i = 0; i < steps.length; i++) {
          const step = steps[i];

          log.info(`[${i + 1}/${steps.length}] ${step.config.name}`);

          await step.run({
            desktop: desktopInstance,
            input: validatedInput,
            context,
            logger: log,
          });

          log.info('');
        }

        const duration = Date.now() - startTime;

        log.info('='.repeat(60));
        log.success(`✅ Workflow completed successfully! (${duration}ms)`);
        log.info('='.repeat(60));

        // Call success handler if provided
        if (successHandler) {
          await successHandler({
            input: validatedInput,
            context,
            logger: log,
            duration,
          });
        }
      } catch (error: any) {
        const duration = Date.now() - startTime;

        log.info('');
        log.info('='.repeat(60));
        log.error(`❌ Workflow failed! (${duration}ms)`);
        log.info('='.repeat(60));

        // Find which step failed
        const failedStepIndex = steps.findIndex(s =>
          error.message?.includes(s.config.name)
        );

        const failedStep = failedStepIndex >= 0 ? steps[failedStepIndex] : steps[steps.length - 1];

        // Call error handler if provided
        if (errorHandler) {
          try {
            await errorHandler({
              error,
              step: failedStep,
              input: validatedInput,
              context,
              logger: log,
            });
          } catch (handlerError) {
            log.error(`❌ Error handler failed: ${handlerError}`);
          }
        }

        // Rethrow original error
        throw error;
      }
    },

    getMetadata() {
      return {
        name: config.name,
        description: config.description,
        version: config.version,
        input: config.input,
        steps: steps.map(s => s.getMetadata()),
      };
    },
  };
}

/**
 * Creates a workflow builder
 *
 * @example
 * ```typescript
 * const workflow = createWorkflow({
 *   name: 'SAP Login',
 *   input: z.object({
 *     username: z.string(),
 *   }),
 * })
 *   .step(loginStep)
 *   .step(processStep)
 *   .onSuccess(async ({ logger }) => {
 *     logger.success('Done!');
 *   })
 *   .build();
 * ```
 */
export function createWorkflow<TInput = any>(
  config: WorkflowConfig<TInput>
): WorkflowBuilder<TInput> {
  return new WorkflowBuilder(config);
}
