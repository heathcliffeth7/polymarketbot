const CHUNK_LOAD_RECOVERY_SCRIPT = String.raw`
(() => {
  if (window.__dextrabotChunkLoadRecoveryInstalled) return;
  window.__dextrabotChunkLoadRecoveryInstalled = true;

  const key = "dextrabot:chunk-load-recovery:" + window.location.pathname;
  const chunkPattern =
    /ChunkLoadError|Loading chunk .* failed|Failed to load chunk|dynamically imported module/i;

  function text(value) {
    if (!value) return "";
    if (value instanceof Error) return value.name + " " + value.message;
    if (typeof value === "string") return value;
    return [value.name, value.message, value.reason].filter(Boolean).join(" ");
  }

  function isChunkResourceFailure(event) {
    const target = event && event.target;
    return (
      target &&
      target.tagName === "SCRIPT" &&
      typeof target.src === "string" &&
      target.src.includes("/_next/static/chunks/")
    );
  }

  function recover(event) {
    const isChunkError =
      chunkPattern.test(text(event && event.error)) ||
      chunkPattern.test(text(event && event.message)) ||
      chunkPattern.test(text(event && event.reason)) ||
      isChunkResourceFailure(event);
    if (!isChunkError) return;

    try {
      if (window.sessionStorage.getItem(key) === "1") return;
      window.sessionStorage.setItem(key, "1");
    } catch (_) {
      return;
    }

    const url = new URL(window.location.href);
    url.searchParams.set("_chunk_retry", Date.now().toString(36));
    window.location.replace(url.toString());
  }

  window.addEventListener("error", recover, true);
  window.addEventListener("unhandledrejection", recover);
})();
`;

export function ChunkLoadRecovery() {
  return (
    <script
      dangerouslySetInnerHTML={{ __html: CHUNK_LOAD_RECOVERY_SCRIPT }}
      suppressHydrationWarning
    />
  );
}
