import { reactive, ref } from 'vue';
import { emptyState, type StateDto } from './types';
import { getAppVersion, getState, onStateUpdated } from './tauri';

// Single module-level reactive snapshot mirroring the backend 1:1 — matches
// the original app.js architecture (one global `state` object), just with
// Vue's fine-grained reactivity replacing the old full-DOM-rebuild `render()`.
export const state = reactive<StateDto>(emptyState());
export const appVersion = ref('');

let initialized = false;

export async function initState(): Promise<void> {
  if (initialized) return;
  initialized = true;

  try {
    appVersion.value = await getAppVersion();
  } catch (e) {
    console.error('Failed to get app version:', e);
  }

  try {
    const s = await getState();
    Object.assign(state, s);
  } catch (e) {
    console.error('Failed to get initial state:', e);
  }

  try {
    await onStateUpdated(s => {
      Object.assign(state, s);
    });
  } catch (e) {
    console.error('Failed to subscribe to state updates:', e);
  }
}
