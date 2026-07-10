<script lang="ts">
  import ThemeProvider from "$lib/components/ThemeProvider.svelte";

  let { children } = $props();

  /** Desktop shell fallback: suppress the native WKWebView context menu ("Reload" etc.) on any
   * surface that didn't already show its own menu. Editable text still gets the OS menu (copy/
   * paste/spellcheck), everything else in the app-provided ones opts in by calling
   * preventDefault() themselves before this listener runs. */
  function suppressDefaultContextMenu(e: MouseEvent) {
    if (e.defaultPrevented) return;
    const target = e.target as HTMLElement | null;
    if (target?.closest("input, textarea, [contenteditable='true']")) return;
    e.preventDefault();
  }
</script>

<svelte:window oncontextmenu={suppressDefaultContextMenu} />

<ThemeProvider>
  {@render children()}
</ThemeProvider>
