import { readFileSync } from 'node:fs';
export const registry = JSON.parse(readFileSync(new URL('../assets/model-registry.json', import.meta.url), 'utf8'));
export const modelIds = Object.fromEntries(registry.tiers.map(model => [model.tier, model.modelId]));
