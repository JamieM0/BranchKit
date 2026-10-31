/** Contain keyboard focus in the topmost dialog and return it to the invoking control. */
const dialogs: HTMLElement[] = [];
const focusable = 'button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])';

export function modal(node: HTMLElement) {
  const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  dialogs.push(node);
  node.tabIndex = -1;
  const controls = () => Array.from(node.querySelectorAll<HTMLElement>(focusable))
    .filter((element) => element.getClientRects().length > 0 && !element.closest('[inert]'));
  const focusFirst = () => (controls()[0] ?? node).focus({ preventScroll: true });
  queueMicrotask(() => {
    if (node.isConnected && dialogs.at(-1) === node && !node.contains(document.activeElement)) focusFirst();
  });

  function onKeydown(event: KeyboardEvent) {
    if (dialogs.at(-1) !== node) return;
    // The dialog's own handlers run first. App-wide shortcuts must not act on the repository
    // behind a dialog, including when focus is on a button rather than a text field.
    event.stopPropagation();
    if (event.key !== 'Tab') return;
    const elements = controls();
    const first = elements[0] ?? node;
    const last = elements.at(-1) ?? node;
    if (elements.length === 0 || !node.contains(document.activeElement) ||
      (event.shiftKey && (document.activeElement === first || document.activeElement === node)) ||
      (!event.shiftKey && document.activeElement === last)) {
      event.preventDefault();
      (event.shiftKey ? last : first).focus();
    }
  }
  function onFocus(event: FocusEvent) {
    if (dialogs.at(-1) === node && !node.contains(event.target as Node)) focusFirst();
  }
  document.addEventListener('keydown', onKeydown);
  document.addEventListener('focusin', onFocus);
  return {
    destroy() {
      const wasTop = dialogs.at(-1) === node;
      const index = dialogs.indexOf(node);
      if (index !== -1) dialogs.splice(index, 1);
      document.removeEventListener('keydown', onKeydown);
      document.removeEventListener('focusin', onFocus);
      queueMicrotask(() => {
        if (!wasTop || (document.activeElement !== document.body && document.activeElement !== null)) return;
        if (previous?.isConnected) previous.focus({ preventScroll: true });
        else dialogs.at(-1)?.focus({ preventScroll: true });
      });
    },
  };
}
