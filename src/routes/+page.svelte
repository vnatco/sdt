<script lang="ts">
  // One page, two windows: the timer and its pop-up menu.
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import App from "$lib/App.svelte";
  import MenuWindow from "$lib/MenuWindow.svelte";

  const isMenu = getCurrentWindow().label === "menu";
</script>

{#if isMenu}
  <MenuWindow />
{:else}
  <App />
{/if}
