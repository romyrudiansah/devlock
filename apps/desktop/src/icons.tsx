// Ikon garis sederhana bergaya Fluent (20×20, mengikuti `currentColor`).

const PATHS = {
  home: "M3.5 9 10 3.5 16.5 9v7a1 1 0 0 1-1 1h-3.5v-5h-4v5H4.5a1 1 0 0 1-1-1z",
  services: "M3 4.5h14v4H3zM3 11.5h14v4H3zM6 6.5h.01M6 13.5h.01",
  projects: "M2.5 5.5a1 1 0 0 1 1-1h4l1.5 1.5h7.5a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1h-13a1 1 0 0 1-1-1z",
  runtimes: "M10 2.5 16.5 6v8L10 17.5 3.5 14V6zM3.5 6 10 9.5 16.5 6M10 9.5v8",
  php: "M7 6 3 10l4 4M13 6l4 4-4 4M11.2 4.5 8.8 15.5",
  settings:
    "M10 12.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5zM16.2 11.6l1.3 1-1.5 2.6-1.6-.5a6 6 0 0 1-1.5.9L12.5 17h-3l-.4-1.6a6 6 0 0 1-1.5-.9l-1.6.5-1.5-2.6 1.3-1a6 6 0 0 1 0-1.8l-1.3-1L5.9 6l1.6.5a6 6 0 0 1 1.5-.9L9.5 4h3l.4 1.6a6 6 0 0 1 1.5.9l1.6-.5 1.5 2.6-1.3 1a6 6 0 0 1 0 1.8z",
  search: "M8.5 14.5a6 6 0 1 0 0-12 6 6 0 0 0 0 12zM13 13l4.5 4.5",
  play: "M6.5 4.5v11l9-5.5z",
  stop: "M5.5 5.5h9v9h-9z",
  restart: "M15.5 10a5.5 5.5 0 1 1-1.6-3.9M15.5 3.5v3h-3",
  folder: "M2.5 5.5a1 1 0 0 1 1-1h4l1.5 1.5h7.5a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1h-13a1 1 0 0 1-1-1z",
  terminal: "M3 4h14v12H3zM6 8l2.5 2L6 12M10 12.5h4",
  code: "M7 6 3 10l4 4M13 6l4 4-4 4",
  globe: "M10 17.5a7.5 7.5 0 1 0 0-15 7.5 7.5 0 0 0 0 15zM2.5 10h15M10 2.5c2 2 3 4.5 3 7.5s-1 5.5-3 7.5c-2-2-3-4.5-3-7.5s1-5.5 3-7.5z",
  log: "M5 2.5h7l3 3v12H5zM8 9h5M8 12h5M8 15h3",
  plus: "M10 4v12M4 10h12",
  database: "M10 7c3.6 0 6.5-1 6.5-2.25S13.6 2.5 10 2.5 3.5 3.5 3.5 4.75 6.4 7 10 7zM3.5 4.75v10.5C3.5 16.5 6.4 17.5 10 17.5s6.5-1 6.5-2.25V4.75M3.5 10c0 1.25 2.9 2.25 6.5 2.25s6.5-1 6.5-2.25",
  arrow: "M4 10h12M11 5l5 5-5 5",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = 20 }: { name: IconName; size?: number }) {
  return (
    <svg
      className="icon"
      width={size}
      height={size}
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
