<script lang="ts">
  let { side, value, onChange, onReset }: {
    side: "left" | "right";
    value: number;
    onChange: (value: number) => void;
    onReset: () => void;
  } = $props();
  let drag: { x: number; width: number } | null = $state(null);
  function keydown(event: KeyboardEvent) {
    if (event.key === "Home") { event.preventDefault(); onReset(); return; }
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const direction = (event.key === "ArrowRight" ? 1 : -1) * (side === "left" ? 1 : -1);
    onChange(value + direction * (event.shiftKey ? 40 : 10));
  }
</script>

<!-- WAI-ARIA window splitter: a focusable separator supports pointer and arrow-key resizing. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="resize-handle" class:dragging={drag !== null} class:left-edge={side === "right"}
  role="separator" aria-orientation="vertical" aria-label="Resize {side} panel"
  aria-valuemin={220} aria-valuemax={480} aria-valuenow={Math.round(value)} tabindex="0"
  title="Drag to resize · Double-click to reset"
  onkeydown={keydown}
  ondblclick={onReset}
  onpointerdown={(event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.focus({ preventScroll: true });
    event.currentTarget.setPointerCapture(event.pointerId);
    drag = { x: event.clientX, width: value };
  }}
  onpointermove={(event) => {
    if (drag) onChange(drag.width + (event.clientX - drag.x) * (side === "left" ? 1 : -1));
  }}
  onpointerup={() => { drag = null; }}
  onpointercancel={() => { drag = null; }}
  onlostpointercapture={() => { drag = null; }}
></div>

<style>
  .resize-handle { position: absolute; top: 0; bottom: 0; right: 0; width: 5px; z-index: 5; cursor: col-resize; touch-action: none; }
  .left-edge { right: auto; left: 0; }
  .resize-handle:hover, .resize-handle.dragging { background: color-mix(in srgb, var(--accent) 40%, transparent); }
  .resize-handle:focus-visible { outline-offset: -2px; }
</style>
