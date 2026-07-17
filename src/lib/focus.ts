/** Focus a newly-mounted control without the browser `autofocus` attribute.
 *
 * Used only for user-opened menus/dialogs, where moving focus into the new surface is expected.
 * Deferring one microtask lets Svelte finish attaching the node before focus is requested.
 */
export function focusOnMount(node: HTMLElement) {
	queueMicrotask(() => node.focus());
}
