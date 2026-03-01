import type { Stagehand } from "@browserbasehq/stagehand";

export interface AgentInput {
  instruction: string;
  maxSteps?: number;
}

export interface AgentResult {
  success: boolean;
  message: string;
  completed: boolean;
  actions: unknown[];
}

export async function handleAgent(
  stagehand: Stagehand,
  input: AgentInput,
): Promise<AgentResult> {
  const agent = stagehand.agent();
  const result = await agent.execute({
    instruction: input.instruction,
    maxSteps: input.maxSteps ?? 20,
  });
  return {
    success: result.success,
    message: result.message,
    completed: result.completed,
    actions: result.actions,
  };
}
