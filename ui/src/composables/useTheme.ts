import { ref } from 'vue';

type Theme = 'light' | 'dark';

// Module-level singleton — one theme for the whole app's lifetime, same as
// the original's single `currentTheme` variable.
const currentTheme = ref<Theme>(document.documentElement.dataset.theme === 'light' ? 'light' : 'dark');

function apply(theme: Theme) {
  currentTheme.value = theme;
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem('macros-theme', theme);
  } catch (_e) {
    // ignore (e.g. storage disabled)
  }
}

export function useTheme() {
  return {
    currentTheme,
    setTheme: apply,
    toggleTheme: () => apply(currentTheme.value === 'light' ? 'dark' : 'light'),
  };
}
