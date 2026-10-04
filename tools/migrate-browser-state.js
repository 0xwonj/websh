// Run once in the browser console on the site's origin before using the new build.
// This file is never imported by the application. It leaves all IndexedDB data alone.
(() => {
  const changed = [];
  const move = (from, to, normalize = value => value) => {
    const value = localStorage.getItem(from);
    if (value === null) return;
    if (localStorage.getItem(to) === null) localStorage.setItem(to, normalize(value));
    localStorage.removeItem(from); // Only after the new value is safely persisted.
    changed.push(from);
  };
  move('reader.TEXT_SCALE', 'websh.reader.scale', value => {
    const scale = value.trim().toLowerCase();
    return ['extra-large', 'extra_large', 'xl'].includes(scale) ? 'xlarge' : scale;
  });
  move('websh.dino_game.high_score.v1', 'websh.dino.score');
  const aliases = {
    sepia: 'sepia-dark', flexoki: 'sepia-dark', 'flexoki-dark': 'sepia-dark', 'dark-paper': 'sepia-dark',
    black: 'black-ink', ink: 'black-ink', paper: 'black-ink', 'paper-light': 'black-ink', 'flexoki-light': 'black-ink',
    gruvbox: 'gruvbox-dark', tokyonight: 'tokyonight-night', 'tokyo-night': 'tokyonight-night', night: 'tokyonight-night',
    solarized: 'solarized-light', light: 'solarized-light', vampire: 'dracula',
    catppuccin: 'catppuccin-mocha', mocha: 'catppuccin-mocha', latte: 'catppuccin-latte',
    nordic: 'nord', arctic: 'nord', rosepine: 'rose-pine', rose: 'rose-pine', pine: 'rose-pine',
    kanagawa: 'kanagawa-wave', wave: 'kanagawa-wave',
  };
  const theme = localStorage.getItem('user.THEME');
  if (theme !== null) {
    const normalized = theme.trim().toLowerCase().replaceAll('_', '-');
    localStorage.setItem('user.THEME', aliases[normalized] || normalized);
  }
  sessionStorage.removeItem('websh.gh_token');
  return { migratedKeys: changed };
})();
