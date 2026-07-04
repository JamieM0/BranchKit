<script lang="ts">
  import ThemeProvider from "$lib/components/ThemeProvider.svelte";

  let { children } = $props();
</script>

<ThemeProvider>
  {@render children()}
</ThemeProvider>
