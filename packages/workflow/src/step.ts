import {
  Step,
  StepConfig,
  StepContext,
  ErrorContext,
  ErrorRecoveryResult,
  ExpectationContext,
  ExpectationResult,
  ExecuteError,
  StepResult,
} from './types';

/**
 * Creates a workflow step with optional type inference for state
 *
 * @example
 * ```typescript
 * // Basic step
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
 *
 * // Step with typed state
 * const processData = createStep<MyInput, MyOutput, { userId: string }, { processedCount: number }>({
 *   id: 'process',
 *   name: 'Process Data',
 *   execute: async ({ context }) => {
 *     const id = context.state.userId; // TypeScript knows this is a string
 *     return { state: { processedCount: 42 } };
 *   }
 * });
 * ```
 */
export function createStep<
  TInput = any,
  TOutput = any,
  TStateIn extends Record<string, any> = Record<string, any>,
  TStateOut extends Record<string, any> = Record<string, any>
>(
  config: StepConfig<TInput, TOutput, TStateIn, TStateOut>
): Step<TInput, TOutput, TStateIn, TStateOut> {
  return {
    config,

    async run(context: StepContext<TInput, TStateIn>): Promise<TOutput | void> {
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
        let result: StepResult<TOutput> | TOutput | void;

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

        // Normalize result to StepResult format
        let normalizedResult: StepResult<TOutput, TStateOut> | void;

        if (result === undefined || result === null) {
          normalizedResult = undefined;
        } else if (typeof result === 'object' && ('data' in result || 'state' in result)) {
          // Already a StepResult
          normalizedResult = result as StepResult<TOutput, TStateOut>;
        } else {
          // Plain object - wrap it as state updates for backward compatibility
          normalizedResult = { state: result as any };
        }

        // Merge state updates into context
        if (normalizedResult && normalizedResult.state) {
          Object.assign(context.context.state, normalizedResult.state);
        }

        // Store data in context
        if (normalizedResult && normalizedResult.data !== undefined) {
          context.context.data[config.id] = normalizedResult.data;
        }

        // Run expectation validation if provided
        if (config.expect) {
          logger.info(`🔍 Validating expectations for: ${config.name}`);

          const expectContext: ExpectationContext<TInput, TOutput, TStateIn> = {
            desktop: context.desktop,
            input: context.input,
            result: normalizedResult?.data as TOutput,
            context: context.context,
            logger: context.logger,
          };

          const expectResult = await config.expect(expectContext);

          if (!expectResult.success) {
            const errorMsg = expectResult.message || 'Expectation not met';
            logger.error(`❌ Expectation failed: ${errorMsg}`);
            throw new Error(`Expectation failed: ${errorMsg}`);
          }

          logger.success(`✅ Expectations met: ${expectResult.message || 'Success'}`);
        }

        const duration = Date.now() - startTime;
        logger.success(`✅ Completed step: ${config.name} (${duration}ms)`);

        return normalizedResult?.data as TOutput;
      } catch (error: any) {
        const duration = Date.now() - startTime;
        logger.error(`❌ Step failed: ${config.name} (${duration}ms)`);
        logger.error(`   Error: ${error.message}`);

        // Enrich error with step metadata if not already present
        if (!error.metadata) {
          error.metadata = {
            step: config.name,
            stepId: config.id,
            duration,
            timestamp: new Date().toISOString(),
          };
        }

        // Try error recovery if handler provided
        if (config.onError) {
          const errorContext: ErrorContext<TInput, TOutput, TStateIn> = {
            error,
            desktop: context.desktop,
            input: context.input,
            context: context.context,
            logger: context.logger,
            attempt: 0,
            retry: async () => {
              logger.info(`🔄 Retrying step: ${config.name}...`);
              const result = await this.run(context);
              return result as TOutput;
            },
          };

          const recoveryResult = await config.onError(errorContext);

          if (recoveryResult && !recoveryResult.recoverable) {
            logger.error(`❌ Cannot recover: ${recoveryResult.reason || 'Unknown'}`);

            // Enrich error with recovery information
            error.recoverable = false;
            if (recoveryResult.reason && !error.code) {
              error.code = 'RECOVERY_FAILED';
            }

            throw error;
          }

          // If onError returned void or recoverable: true, it handled the retry
          return;
        }

        // No error handler - mark as non-recoverable and rethrow
        if (error.recoverable === undefined) {
          error.recoverable = false;
        }

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
