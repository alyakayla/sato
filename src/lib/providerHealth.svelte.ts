import { api } from './api';
import type { ProviderProblemKind, ProviderStatus } from './types';

/**
 * The AI provider's health, shared by the chat banner, failed replies and
 * Settings. Checked when the chat opens, after a turn fails, and on demand —
 * so a missing Ollama is explained before the user asks anything, not after.
 */

export const providerHealth = $state<{ status: ProviderStatus | null; checking: boolean }>({
  status: null,
  checking: false,
});

export async function checkProvider(): Promise<ProviderStatus | null> {
  if (providerHealth.checking) return providerHealth.status;
  providerHealth.checking = true;
  try {
    providerHealth.status = await api.testProvider();
  } catch {
    // The check itself failing (not the provider) leaves the last result.
  } finally {
    providerHealth.checking = false;
  }
  return providerHealth.status;
}

export function setProviderStatus(status: ProviderStatus): void {
  providerHealth.status = status;
}

/** What the diagnosis component needs, from a status or a failed reply. */
export interface ProblemInfo {
  problem: ProviderProblemKind;
  provider: string;
  url: string;
  chatModel: string;
  embeddingModel: string;
  missingModels: string[];
  detail: string;
}

export function problemFromStatus(s: ProviderStatus): ProblemInfo | null {
  if (s.configured || !s.problem) return null;
  return {
    problem: s.problem,
    provider: s.name,
    url: s.baseUrl,
    chatModel: s.chatModel,
    embeddingModel: s.embeddingModel,
    missingModels: s.missingModels,
    detail: s.detail,
  };
}

/**
 * A failed reply stores a JSON diagnosis in `error`. Older replies (and other
 * failures) hold plain text; those return null and render as text.
 */
export function problemFromReply(error: string): ProblemInfo | null {
  if (!error.startsWith('{')) return null;
  try {
    const e = JSON.parse(error) as {
      problem?: ProviderProblemKind;
      provider?: string;
      url?: string;
      model?: string;
      embeddingModel?: string;
      detail?: string;
    };
    if (!e.problem) return null;
    return {
      problem: e.problem,
      provider: e.provider ?? 'ollama',
      url: e.url ?? '',
      chatModel: e.model ?? '',
      embeddingModel: e.embeddingModel ?? '',
      missingModels: e.problem === 'model_missing' && e.model ? [e.model] : [],
      detail: e.detail ?? '',
    };
  } catch {
    return null;
  }
}
