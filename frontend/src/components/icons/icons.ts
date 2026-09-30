// A single hand-drawn icon system for the whole app, on one 20x20 grid with
// one stroke weight (1.5) and one cap/join style (round), so every icon -
// nav, actions, theme, and the editor toolbar - reads as one family instead
// of the mix of ad-hoc inline SVGs and plain text glyphs ("B", "H1", "•")
// this replaces. Each entry is the *inner* markup only; Icon.vue supplies
// the shared <svg> wrapper (viewBox, stroke/fill defaults via currentColor).
//
// A few icons (pin, folder, moon, and the toolbar glyphs that read better
// solid at this size - bold, bullet dots) are filled rather than stroked;
// that's a deliberate, consistent "filled = state/emphasis, stroked =
// action" split, not an inconsistency.
export const icons = {
  search: `
    <circle cx="8.5" cy="8.5" r="5.5" />
    <path d="M13 13l4.5 4.5" stroke-linecap="round" />
  `,
  sparkle: `
    <path d="M8.5 2.5c.28 0 .52.2.55.5l.7 3.9 3.9.7a.55.55 0 0 1 0 1.08l-3.9.7-.7 3.9a.55.55 0 0 1-1.1 0l-.7-3.9-3.9-.7a.55.55 0 0 1 0-1.08l3.9-.7.7-3.9a.55.55 0 0 1 .55-.5Z" fill="currentColor" stroke="none" />
    <path d="M15 11.5c.22 0 .4.15.44.36l.35 1.9 1.9.35a.45.45 0 0 1 0 .88l-1.9.35-.35 1.9a.45.45 0 0 1-.88 0l-.35-1.9-1.9-.35a.45.45 0 0 1 0-.88l1.9-.35.35-1.9a.45.45 0 0 1 .44-.36Z" fill="currentColor" stroke="none" />
  `,
  notesList: `
    <rect x="3" y="3" width="14" height="14" rx="3" />
    <path d="M6.5 7.5h7M6.5 10h7M6.5 12.5h4.5" stroke-linecap="round" />
  `,
  plus: `
    <path d="M10 4v12M4 10h12" stroke-linecap="round" />
  `,
  folder: `
    <path d="M3 5.5A1.5 1.5 0 0 1 4.5 4h3.379a1.5 1.5 0 0 1 1.06.44l1.122 1.12a1.5 1.5 0 0 0 1.06.44H15.5A1.5 1.5 0 0 1 17 7.5v7A1.5 1.5 0 0 1 15.5 16h-11A1.5 1.5 0 0 1 3 14.5v-9Z" fill="currentColor" stroke="none" />
  `,
  trash: `
    <path d="M4 6.5h12M8.25 6.5V5a1 1 0 0 1 1-1h1.5a1 1 0 0 1 1 1v1.5" stroke-linecap="round" stroke-linejoin="round" />
    <path d="M6.5 6.5l.6 8.1A1.5 1.5 0 0 0 8.6 16h2.8a1.5 1.5 0 0 0 1.5-1.4l.6-8.1" stroke-linecap="round" stroke-linejoin="round" />
    <path d="M8.5 9.5v4M11.5 9.5v4" stroke-linecap="round" />
  `,
  restore: `
    <path d="M4 10a6 6 0 1 1 2 4.5M4 10V6M4 10h4" stroke-linecap="round" stroke-linejoin="round" />
  `,
  moon: `
    <path d="M10 2.5a7.5 7.5 0 1 0 7.35 9.02.75.75 0 0 0-.9-.88 5.8 5.8 0 0 1-7.09-7.09.75.75 0 0 0-.88-.9c-.36.06-.72.13-1.06.23A7.53 7.53 0 0 0 10 2.5Z" fill="currentColor" stroke="none" />
  `,
  sun: `
    <circle cx="10" cy="10" r="3.5" fill="currentColor" stroke="none" />
    <path d="M10 2v2M10 16v2M18 10h-2M4 10H2M15.36 4.64l-1.42 1.42M6.06 13.94l-1.42 1.42M15.36 15.36l-1.42-1.42M6.06 6.06 4.64 4.64" stroke-width="1.3" stroke-linecap="round" />
  `,
  monitor: `
    <rect x="2.5" y="4.5" width="15" height="10" rx="1.5" stroke-width="1.4" />
    <path d="M7 17.5h6" stroke-width="1.4" stroke-linecap="round" />
  `,
  panelLeft: `
    <rect x="3" y="4" width="14" height="12" rx="2" />
    <path d="M8 4v12" />
  `,
  panelList: `
    <rect x="3" y="4" width="14" height="12" rx="2" />
    <path d="M6.5 7.5h7M6.5 10h7M6.5 12.5h4.5" stroke-linecap="round" />
  `,
  download: `
    <path d="M10 3v10M6 9.5 10 13.5 14 9.5" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
    <path d="M4 16.5h12" stroke-width="1.6" stroke-linecap="round" />
  `,
  pin: `
    <path d="M11.5 2.5a1 1 0 0 1 1.4 0l4.6 4.6a1 1 0 0 1 0 1.4l-.7.7a1 1 0 0 1-1.4 0l-.2-.2-2.6 2.6.6 2.9a.75.75 0 0 1-1.27.68l-2.9-2.9-4 4a.6.6 0 0 1-.85-.85l4-4-2.9-2.9a.75.75 0 0 1 .68-1.27l2.9.6 2.6-2.6-.2-.2a1 1 0 0 1 0-1.4l.7-.7Z" fill="currentColor" stroke="none" />
  `,
  document: `
    <rect x="4.5" y="2.5" width="11" height="15" rx="1.5" stroke-width="1.3" />
    <path d="M7 6.5h6M7 9.5h6M7 12.5h4" stroke-width="1.3" stroke-linecap="round" />
  `,
  sort: `
    <path d="M7 4v12M4.5 6.5 7 4l2.5 2.5" stroke-linecap="round" stroke-linejoin="round" />
    <path d="M13 16V4M10.5 13.5 13 16l2.5-2.5" stroke-linecap="round" stroke-linejoin="round" />
  `,
  check: `
    <path d="M4.5 10.5 8 14l7.5-9" stroke-linecap="round" stroke-linejoin="round" />
  `,
  close: `
    <path d="M5.5 5.5l9 9M14.5 5.5l-9 9" stroke-linecap="round" />
  `,
  chevronUp: `
    <path d="M5.5 12.5 10 8l4.5 4.5" stroke-linecap="round" stroke-linejoin="round" />
  `,
  chevronDown: `
    <path d="M5.5 7.5 10 12l4.5-4.5" stroke-linecap="round" stroke-linejoin="round" />
  `,
  // Editor toolbar - previously plain text glyphs ("B", "I", "H1", "•", ...)
  bold: `
    <path d="M6.5 4.5h4.3a2.6 2.6 0 0 1 0 5.2H6.5V4.5Z" fill="currentColor" stroke="none" />
    <path d="M6.5 9.7h4.9a2.8 2.8 0 0 1 0 5.6H6.5V9.7Z" fill="currentColor" stroke="none" />
  `,
  italic: `
    <path d="M8.5 4.5h5M6.5 15.5h5M11.5 4.5l-3 11" stroke-linecap="round" />
  `,
  underline: `
    <path d="M6 4v6a4 4 0 0 0 8 0V4" stroke-linecap="round" />
    <path d="M4.5 16.5h11" stroke-linecap="round" />
  `,
  strikethrough: `
    <path d="M6.3 6.5c0-1.4 1.5-2.5 3.7-2.5s3.7 1 3.9 2.3" stroke-linecap="round" />
    <path d="M6.1 13.5c.1 1.4 1.7 2.5 3.9 2.5s3.7-1 3.7-2.6c0-1.1-.7-1.9-1.9-2.4" stroke-linecap="round" />
    <path d="M3.5 10h13" stroke-linecap="round" />
  `,
  highlight: `
    <path d="M12.5 3.5 16.5 7.5 9 15H5v-4l7.5-7.5Z" stroke-linejoin="round" />
    <path d="M4 18h5" stroke-linecap="round" />
  `,
  heading1: `
    <path d="M4.5 4.5v11M4.5 10h6M10.5 4.5v11" stroke-linecap="round" />
    <path d="M13.5 8.2 15.2 7v7" stroke-linecap="round" stroke-linejoin="round" />
  `,
  heading2: `
    <path d="M4.5 4.5v11M4.5 10h6M10.5 4.5v11" stroke-linecap="round" />
    <path d="M13.3 8.4c0-1 .9-1.7 1.9-1.7s1.9.7 1.9 1.7c0 1.2-1.2 2.2-3.8 4.6h3.8" stroke-linecap="round" stroke-linejoin="round" />
  `,
  checklist: `
    <rect x="3.2" y="4" width="4.4" height="4.4" rx="1" stroke-linejoin="round" />
    <path d="M4.3 6.2 5.3 7.2 6.8 5.4" stroke-linecap="round" stroke-linejoin="round" />
    <path d="M9.5 6.2h7" stroke-linecap="round" />
    <rect x="3.2" y="11.6" width="4.4" height="4.4" rx="1" stroke-linejoin="round" />
    <path d="M9.5 13.8h7" stroke-linecap="round" />
  `,
  bulletList: `
    <circle cx="4.3" cy="6.2" r="1.1" fill="currentColor" stroke="none" />
    <circle cx="4.3" cy="10" r="1.1" fill="currentColor" stroke="none" />
    <circle cx="4.3" cy="13.8" r="1.1" fill="currentColor" stroke="none" />
    <path d="M8 6.2h9M8 10h9M8 13.8h9" stroke-linecap="round" />
  `,
  numberedList: `
    <path d="M3.6 5.3 4.3 4.7v2.8" stroke-linecap="round" stroke-linejoin="round" />
    <path d="M3.5 8.8c0-.6.5-1 1-1s1 .4 1 1c0 .5-.4.9-1.6 2h1.8" stroke-linecap="round" stroke-linejoin="round" />
    <path d="M3.5 12.6c.1-.5.6-.8 1.1-.8s1 .3 1 .8-.4.8-.8.9c.5.1.9.4.9.9s-.5.9-1.1.9-1-.3-1.1-.7" stroke-linecap="round" stroke-linejoin="round" />
    <path d="M8 6.2h9M8 10h9M8 13.8h9" stroke-linecap="round" />
  `,
  code: `
    <path d="M7 6 3 10l4 4M13 6l4 4-4 4" stroke-linecap="round" stroke-linejoin="round" />
  `,
  quote: `
    <path d="M4.5 4v12" stroke-width="2.2" stroke-linecap="round" />
    <path d="M8 6.5h9M8 10h9M8 13.5h6" stroke-linecap="round" />
  `,
} as const;

export type IconName = keyof typeof icons;
