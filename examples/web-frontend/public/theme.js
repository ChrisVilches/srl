// The theme, chosen with the button in the top bar and kept in localStorage;
// dark unless set to light. index.html loads it in its head, before the page
// is drawn, so it doesn't flash the other theme first (src/theme.ts). It is a
// file rather than an inline script so that the server's
// Content-Security-Policy can forbid inline scripts. It also gives a phone's
// status bar the top bar's color, as src/theme.ts's THEME_COLORS.
let dark = true;
try {
  dark = localStorage.getItem('srl-web-theme') !== 'light';
} catch {
  // Not readable, as in some private windows: dark.
}
document.documentElement.classList.toggle('dark', dark);
document.querySelector('meta[name="theme-color"]')?.setAttribute('content', dark ? '#171413' : '#fefefd');
