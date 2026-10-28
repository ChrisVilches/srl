// The theme: dark or light, chosen with the button in the top bar and kept
// in localStorage under KEY. Dark unless set to light; the system's setting
// isn't used. index.html sets it before the page is drawn.
export type Theme = 'dark' | 'light';

const KEY = 'srl-web-theme';

export function currentTheme(): Theme {
  return document.documentElement.classList.contains('dark') ? 'dark' : 'light';
}

export function setTheme(theme: Theme) {
  document.documentElement.classList.toggle('dark', theme === 'dark');
  try {
    localStorage.setItem(KEY, theme);
  } catch {
    // Not kept, as in a private window: the theme lasts until the page reloads.
  }
}
