export interface ThemeDescriptor {
  name: string
}

export const themes: ReadonlyArray<ThemeDescriptor> = [
  // Markd: Microsoft Word style
  { name: 'word' },
  { name: 'word-dark' },
  // Light Themes (alphabetical)
  { name: 'ayu-light' },
  { name: 'light' },
  { name: 'catppuccin-latte' },
  { name: 'everforest-light' },
  { name: 'graphite' },
  { name: 'gruvbox-light' },
  { name: 'rose-pine-dawn' },
  { name: 'solarized-light' },
  { name: 'tokyo-night-light' },
  { name: 'ulysses' },
  { name: 'github-light' },
  { name: 'one-light' },
  { name: 'sepia' },
  { name: 'vitesse-light' },
  { name: 'quiet-light' },
  // Dark Themes (alphabetical)
  { name: 'ayu-dark' },
  { name: 'ayu-mirage' },
  { name: 'dark' },
  { name: 'catppuccin-mocha' },
  { name: 'cyberdream' },
  { name: 'dracula' },
  { name: 'everforest-dark' },
  { name: 'gruvbox-dark' },
  { name: 'horizon-dark' },
  { name: 'kanagawa' },
  { name: 'material-dark' },
  { name: 'monokai-pro' },
  { name: 'nightfox' },
  { name: 'nord' },
  { name: 'one-dark' },
  { name: 'oxocarbon-dark' },
  { name: 'palenight' },
  { name: 'rose-pine' },
  { name: 'rose-pine-moon' },
  { name: 'solarized-dark' },
  { name: 'synthwave-84' },
  { name: 'tokyo-night' },
  { name: 'tokyo-night-storm' },
  { name: 'github-dark' },
  { name: 'night-owl' },
  { name: 'vitesse-dark' },
  { name: 'cobalt2' },
  { name: 'zenburn' },
  { name: 'moonlight' }
]

// getAutoSwitchThemeOptions removed - no longer needed
// We now use a boolean toggle for followSystemTheme instead of a dropdown
