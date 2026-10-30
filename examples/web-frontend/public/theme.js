// The theme, chosen with the button in the top bar and kept in localStorage;
// dark unless set to light. index.html loads it in its head, before the page
// is drawn, so it doesn't flash the other theme first (src/theme.ts). It is a
// file rather than an inline script so that the server's
// Content-Security-Policy can forbid inline scripts.
try {
  document.documentElement.classList.toggle('dark', localStorage.getItem('srl-web-theme') !== 'light');
} catch {
  document.documentElement.classList.add('dark');
}
