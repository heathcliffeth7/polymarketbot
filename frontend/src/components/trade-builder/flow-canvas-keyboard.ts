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
  useEffect(() => {
    handlersRef.current = handlers;
  });

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

    if (ctrl && e.key === 'k') {
      e.preventDefault();
      handlersRef.current.onSearch();
      return;
    }

    if (isInput) return;

    if (ctrl && e.key === 's') {
      e.preventDefault();
      handlersRef.current.onSave();
      return;
    }
    if (ctrl && e.key === 'z' && !e.shiftKey) {
      e.preventDefault();
      handlersRef.current.onUndo();
      return;
    }
    if (ctrl && ((e.key === 'z' && e.shiftKey) || e.key === 'y')) {
      e.preventDefault();
      handlersRef.current.onRedo();
      return;
    }
    if (ctrl && e.key === 'c') {
      e.preventDefault();
      handlersRef.current.onCopy();
      return;
    }
    if (ctrl && e.key === 'v') {
      e.preventDefault();
      handlersRef.current.onPaste();
      return;
    }
    if (ctrl && e.key === 'a') {
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
  }, []);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    el.addEventListener('keydown', onKeyDown);
    return () => el.removeEventListener('keydown', onKeyDown);
  }, [containerRef, onKeyDown]);
}
