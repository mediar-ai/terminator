/**
 * Unit tests for workflow complete() early exit functionality
 */

import { createWorkflow, createStep, z, complete } from "../index";
import type { StepResult } from "../types";

// Mock Desktop for unit tests
const mockDesktop = {
    locator: jest.fn(),
    openApplication: jest.fn(),
    delay: jest.fn(),
} as any;

describe("Workflow Complete Tests", () => {
    describe("complete() early exit", () => {
        test("exits workflow early with success when complete() is thrown", async () => {
            const executionOrder: string[] = [];

            const step1 = createStep({
                id: "step1",
                name: "Step 1",
                execute: async (): Promise<StepResult> => {
                    executionOrder.push("step1");
                    return { state: { step1: true } };
                },
            });

            const step2 = createStep({
                id: "step2",
                name: "Step 2 - exits early",
                execute: async (): Promise<StepResult> => {
                    executionOrder.push("step2");
                    throw complete({
                        message: "No files to process",
                        data: { filesChecked: 0 },
                    });
                },
            });

            const step3 = createStep({
                id: "step3",
                name: "Step 3 - should not run",
                execute: async (): Promise<StepResult> => {
                    executionOrder.push("step3");
                    return { state: { step3: true } };
                },
            });

            const workflow = createWorkflow({
                input: z.object({}),
                steps: [step1, step2, step3],
            });

            const result = await workflow.run({}, mockDesktop);

            expect(result.status).toBe("success");
            expect(result.message).toBe("No files to process");
            expect(result.data).toEqual({
                message: "No files to process",
                data: { filesChecked: 0 },
            });
            expect(executionOrder).toEqual(["step1", "step2"]);
            // step3 should NOT have run
            expect(executionOrder).not.toContain("step3");
        });

        test("complete() bypasses onSuccess handler", async () => {
            let onSuccessCalled = false;

            const step1 = createStep({
                id: "step1",
                name: "Step 1",
                execute: async (): Promise<StepResult> => {
                    throw complete({
                        message: "Early exit",
                        success: true,
                    });
                },
            });

            const workflow = createWorkflow({
                input: z.object({}),
                steps: [step1],
                onSuccess: async () => {
                    onSuccessCalled = true;
                    return { message: "onSuccess ran" };
                },
            });

            const result = await workflow.run({}, mockDesktop);

            expect(result.status).toBe("success");
            expect(result.message).toBe("Early exit");
            expect(onSuccessCalled).toBe(false);
        });

        test("complete() with custom data fields", async () => {
            const step1 = createStep({
                id: "step1",
                name: "Step 1",
                execute: async (): Promise<StepResult> => {
                    throw complete({
                        message: "Custom completion",
                        summary: "# Summary\nNo work needed",
                        data: {
                            outlet: "TEST",
                            date: "2025-01-01",
                        },
                        customField: "customValue",
                    });
                },
            });

            const workflow = createWorkflow({
                input: z.object({}),
                steps: [step1],
            });

            const result = await workflow.run({}, mockDesktop);

            expect(result.status).toBe("success");
            expect(result.data.summary).toBe("# Summary\nNo work needed");
            expect(result.data.data.outlet).toBe("TEST");
            expect(result.data.customField).toBe("customValue");
        });

        test("complete() preserves lastStepId and lastStepIndex", async () => {
            const step1 = createStep({
                id: "first_step",
                name: "First Step",
                execute: async (): Promise<StepResult> => {
                    return { state: {} };
                },
            });

            const step2 = createStep({
                id: "second_step",
                name: "Second Step",
                execute: async (): Promise<StepResult> => {
                    throw complete({ message: "Done early" });
                },
            });

            const workflow = createWorkflow({
                input: z.object({}),
                steps: [step1, step2],
            });

            const result = await workflow.run({}, mockDesktop);

            expect(result.status).toBe("success");
            expect(result.lastStepId).toBe("first_step");
            expect(result.lastStepIndex).toBe(0);
        });
    });
});
