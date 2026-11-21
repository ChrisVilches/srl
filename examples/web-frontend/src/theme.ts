// The theme: dark or light, chosen with the button in the top bar and kept
// in localStorage under KEY. Dark unless set to light; the system's setting
// isn't used. index.html sets it before the page is drawn.
export type Theme = 'dark' | 'light';

const KEY = 'srl-web-theme';

// The color of a phone's status bar, and of an installed app's title bar:
// the top bar's, its background (src/TopBar.tsx) over the page's, worked
// out, since the meta tag takes no CSS. public/theme.js has them too.
const THEME_COLORS: Record<Theme, string> = { dark: '#171413', light: '#fefefd' };

export function currentTheme(): Theme {
  return document.documentElement.classList.contains('dark') ? 'dark' : 'light';
}

export function setTheme(theme: Theme) {
  document.documentElement.classList.toggle('dark', theme === 'dark');
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', THEME_COLORS[theme]);
  try {
    localStorage.setItem(KEY, theme);
  } catch {
    // Not kept, as in a private window: the theme lasts until the page reloads.
  }
}
