import { useCallback, useEffect, useRef } from 'react';

export interface CanvasShortcutHandlers {
  onSave: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onSelectAll: () => void;
  onDeselect: () => void;
  onDelete: () => void;
  onSearch: () => void;
}

export function useCanvasKeyboard(
  handlers: CanvasShortcutHandlers,
  containerRef: React.RefObject<HTMLDivElement | null>
) {
  const handlersRef = useRef(handlers);
  const activeRef = useRef(false);
  useEffect(() => {
    handlersRef.current = handlers;
  });

  const isInsideContainer = useCallback((target: EventTarget | null) => {
    const container = containerRef.current;
    return !!(
      container &&
      target instanceof Node &&
      container.contains(target)
    );
  }, [containerRef]);

  const onKeyDown = useCallback((e: KeyboardEvent) => {
    const target = e.target instanceof HTMLElement ? e.target : null;
    const tagName = target?.tagName.toLowerCase() ?? '';
    const isInput =
      !!target &&
      (target.isContentEditable ||
        tagName === 'input' ||
        tagName === 'textarea' ||
        tagName === 'select');
    const ctrl = e.ctrlKey || e.metaKey;
    const isEditorShortcutActive = activeRef.current || isInsideContainer(e.target);

    if (!isEditorShortcutActive) return;

    if (ctrl && e.key.toLowerCase() === 'k') {
      if (e.repeat) return;
      e.preventDefault();
      handlersRef.current.onSearch();
      return;
    }

    if (isInput) return;

    if (ctrl && e.key.toLowerCase() === 's') {
      if (e.repeat) return;
      e.preventDefault();
      handlersRef.current.onSave();
      return;
    }
    if (ctrl && e.key.toLowerCase() === 'z' && !e.shiftKey) {
      if (e.repeat) return;
      e.preventDefault();
      handlersRef.current.onUndo();
      return;
    }
    if (ctrl && ((e.key.toLowerCase() === 'z' && e.shiftKey) || e.key.toLowerCase() === 'y')) {
      if (e.repeat) return;
      e.preventDefault();
      handlersRef.current.onRedo();
      return;
    }
    if (ctrl && e.key.toLowerCase() === 'c') {
      if (e.repeat) return;
      e.preventDefault();
      handlersRef.current.onCopy();
      return;
    }
    if (ctrl && e.key.toLowerCase() === 'v') {
      if (e.repeat) return;
      e.preventDefault();
      handlersRef.current.onPaste();
      return;
    }
    if (ctrl && e.key.toLowerCase() === 'a') {
      if (e.repeat) return;
      e.preventDefault();
      handlersRef.current.onSelectAll();
      return;
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      handlersRef.current.onDeselect();
      return;
    }
    if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      handlersRef.current.onDelete();
      return;
    }
  }, [isInsideContainer]);

  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      activeRef.current = isInsideContainer(event.target);
    };

    const onFocusIn = (event: FocusEvent) => {
      activeRef.current = isInsideContainer(event.target);
    };

    window.addEventListener('keydown', onKeyDown);
    document.addEventListener('pointerdown', onPointerDown, true);
    document.addEventListener('focusin', onFocusIn, true);

    return () => {
      window.removeEventListener('keydown', onKeyDown);
      document.removeEventListener('pointerdown', onPointerDown, true);
      document.removeEventListener('focusin', onFocusIn, true);
    };
  }, [isInsideContainer, onKeyDown]);
}
