<script lang="ts">
  import { theme } from "$lib/stores/theme.svelte";
  import "$lib/tokens.css";

  let { children } = $props();

  $effect(() => {
    document.documentElement.setAttribute("data-theme", theme.resolved);
  });
</script>

{@render children()}
