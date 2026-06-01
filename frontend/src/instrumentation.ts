export function register() {
  // Server-only claim sweep code must not be imported from Next instrumentation,
  // because this hook is also compiled for the Edge runtime.
}
