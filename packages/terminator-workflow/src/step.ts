import {
  Step,
  StepConfig,
  StepContext,
  ErrorContext,
  ErrorRecoveryResult,
} from './types';

/**
 * Creates a workflow step
 *
 * @example
 * ```typescript
 * const login = createStep({
 *   id: 'login',
 *   name: 'Login to Application',
 *   execute: async ({ desktop, input }) => {
 *     await desktop.locator('role:textbox').fill(input.username);
 *     await desktop.locator('role:button').click();
 *   },
 *   onError: async ({ error, retry }) => {
 *     if (error.message.includes('Session conflict')) {
 *       return retry();
 *     }
 *   }
 * });
 * ```
 */
export function createStep<TInput = any, TOutput = any>(
  config: StepConfig<TInput, TOutput>
): Step<TInput, TOutput> {
  return {
    config,

    async run(context: StepContext<TInput>): Promise<TOutput | void> {
      const { logger } = context;
      const startTime = Date.now();

      try {
        // Check condition if provided
        if (config.condition) {
          const shouldRun = config.condition({
            input: context.input,
            context: context.context,
          });

          if (!shouldRun) {
            logger.info(`⏭️  Skipping step: ${config.name} (condition not met)`);
            return;
          }
        }

        logger.info(`▶️  Executing step: ${config.name}`);

        // Execute with timeout if specified
        let result: TOutput | void;

        if (config.timeout) {
          result = await Promise.race([
            config.execute(context),
            new Promise<never>((_, reject) =>
              setTimeout(
                () => reject(new Error(`Step timeout after ${config.timeout}ms`)),
                config.timeout
              )
            ),
          ]);
        } else {
          result = await config.execute(context);
        }

        const duration = Date.now() - startTime;
        logger.success(`✅ Completed step: ${config.name} (${duration}ms)`);

        return result;
      } catch (error: any) {
        const duration = Date.now() - startTime;
        logger.error(`❌ Step failed: ${config.name} (${duration}ms)`);
        logger.error(`   Error: ${error.message}`);

        // Try error recovery if handler provided
        if (config.onError) {
          const errorContext: ErrorContext<TInput, TOutput> = {
            error,
            desktop: context.desktop,
            input: context.input,
            context: context.context,
            logger: context.logger,
            attempt: 0,
            retry: async () => {
              logger.info(`🔄 Retrying step: ${config.name}...`);
              return this.run(context);
            },
          };

          const recoveryResult = await config.onError(errorContext);

          if (recoveryResult && !recoveryResult.recoverable) {
            logger.error(`❌ Cannot recover: ${recoveryResult.reason || 'Unknown'}`);
            throw error;
          }

          // If onError returned void or recoverable: true, it handled the retry
          return;
        }

        // No error handler - rethrow
        throw error;
      }
    },

    getMetadata() {
      return {
        id: config.id,
        name: config.name,
        description: config.description,
      };
    },
  };
}
