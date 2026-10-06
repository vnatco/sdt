<script lang="ts" module>
  // The app's icons, drawn on a 24 px grid with round strokes.
  const PATHS: Record<string, string> = {
    shutdown: "M12 3.5v7.5 M7.05 6.4a7.5 7.5 0 1 0 9.9 0",
    restart: "M19.5 12a7.5 7.5 0 1 1-2.2-5.3 M19.6 4.2v4.4h-4.4",
    sleep: "M19.2 14.6A7.6 7.6 0 0 1 9.4 4.8a7.6 7.6 0 1 0 9.8 9.8Z",
    pause: "M9 6.5v11 M15 6.5v11",
    play: "M8.5 6.2v11.6c0 .7.8 1.2 1.4.8l8.6-5.8c.5-.4.5-1.2 0-1.6L9.9 5.4c-.6-.4-1.4.1-1.4.8Z",
    plus: "M12 6v12 M6 12h12",
    x: "M7 7l10 10 M17 7 7 17",
    pill: "M5 10.5h14a3 3 0 0 1 0 6H5a3 3 0 0 1 0-6Z M9 6h6",
    expand: "M8 4.5H5.5a1 1 0 0 0-1 1V8 M16 4.5h2.5a1 1 0 0 1 1 1V8 M8 19.5H5.5a1 1 0 0 1-1-1V16 M16 19.5h2.5a1 1 0 0 0 1-1V16",
    tray: "M12 4v9.5 M8.2 9.8 12 13.6l3.8-3.8 M4.5 15.5v2a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2v-2",
    pin: "M14.6 4.5 19.5 9.4 M15.8 5.7l-4.2 4.2-3.9-.6-1.9 1.9 6.9 6.9 1.9-1.9-.6-3.9 4.2-4.2 M9.4 14.6 5 19",
    close: "M7 7l10 10 M17 7 7 17",
    check: "M5.5 12.5 10 17l8.5-9.5",
    reset: "M5 12a7 7 0 1 0 2.1-5 M5 4.5V9h4.5",
    info: "M12 11v5.5 M12 7.6v.1 M12 20.5a8.5 8.5 0 1 0 0-17 8.5 8.5 0 0 0 0 17Z",
    clock: "M12 7.5V12l3 2 M12 20.5a8.5 8.5 0 1 0 0-17 8.5 8.5 0 0 0 0 17Z",
    hourglass: "M7 4h10 M7 20h10 M8 4c0 4 8 4 8 8s-8 4-8 8 M16 4c0 4-8 4-8 8s8 4 8 8",
    sound: "M5 9.5v5h3.2l4.3 3.8V5.7L8.2 9.5H5Z M16 9a4.2 4.2 0 0 1 0 6 M18.5 6.5a7.8 7.8 0 0 1 0 11",
    github: "M9.2 19.6c-4 1.2-4-2-5.6-2.4m11.2 4.8v-3.1c0-.9.1-1.6-.4-2.1 2.5-.3 5.2-1.2 5.2-5.6a4.4 4.4 0 0 0-1.2-3 4 4 0 0 0-.1-3s-1-.3-3.1 1.2a10.7 10.7 0 0 0-5.6 0C6.5 5 5.5 5.3 5.5 5.3a4 4 0 0 0-.1 3 4.4 4.4 0 0 0-1.2 3c0 4.4 2.7 5.3 5.2 5.6-.5.5-.5 1.2-.4 2.1v3.1",
    quit: "M14 4.5h3.5a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H14 M10.5 8 6.5 12l4 4 M6.8 12H15",
    eye: "M2.8 12s3.4-6.2 9.2-6.2 9.2 6.2 9.2 6.2-3.4 6.2-9.2 6.2S2.8 12 2.8 12Z M12 14.6a2.6 2.6 0 1 0 0-5.2 2.6 2.6 0 0 0 0 5.2Z",
  };
  const FILLED = new Set(["play", "sleep"]);
</script>

<script lang="ts">
  let { name, size = 18, stroke = 1.8, fill = false }: { name: string; size?: number; stroke?: number; fill?: boolean } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
  <path
    d={PATHS[name] ?? ""}
    fill={fill && FILLED.has(name) ? "currentColor" : "none"}
    stroke="currentColor"
    stroke-width={stroke}
    stroke-linecap="round"
    stroke-linejoin="round"
  />
</svg>

<style>
  svg {
    display: block;
    flex: none;
  }
</style>
