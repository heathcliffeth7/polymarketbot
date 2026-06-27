'use client';

export type ClientRequestErrorKind = 'network' | 'timeout' | 'http' | 'parse';

interface ClientRequestErrorOptions {
  kind: ClientRequestErrorKind;
  endpoint: string;
  status?: number;
  apiCode?: string;
  retryable?: boolean;
  cause?: unknown;
}

export class ClientRequestError extends Error {
  readonly kind: ClientRequestErrorKind;
  readonly endpoint: string;
  readonly status?: number;
  readonly apiCode?: string;
  readonly retryable?: boolean;
  readonly cause?: unknown;

  constructor(message: string, options: ClientRequestErrorOptions) {
    super(message);
    this.name = 'ClientRequestError';
    this.kind = options.kind;
    this.endpoint = options.endpoint;
    this.status = options.status;
    this.apiCode = options.apiCode;
    this.retryable = options.retryable;
    this.cause = options.cause;
  }
}

export interface RequestJsonOptions {
  timeoutMs?: number;
  retries?: number;
  retryDelayMs?: number;
}

const DEFAULT_TIMEOUT_MS = 10_000;
const DEFAULT_RETRY_DELAY_MS = 300;

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function isAbortError(err: unknown): boolean {
  return err instanceof DOMException && err.name === 'AbortError';
}

function normalizeClientError(
  err: unknown,
  endpoint: string,
  timeoutMs: number
): ClientRequestError {
  if (err instanceof ClientRequestError) return err;

  if (isAbortError(err)) {
    return new ClientRequestError(
      `Istek zaman asimina ugradi (${Math.round(timeoutMs / 1000)}s). Tekrar dene.`,
      { kind: 'timeout', endpoint, cause: err }
    );
  }

  const rawMessage = err instanceof Error ? err.message : String(err ?? '');
  if (
    rawMessage.includes('Failed to fetch') ||
    rawMessage.includes('NetworkError') ||
    rawMessage.includes('Load failed')
  ) {
    return new ClientRequestError(
      'Sunucuya ulasilamadi. Baglantiyi kontrol edip tekrar dene.',
      { kind: 'network', endpoint, cause: err }
    );
  }

  return new ClientRequestError(rawMessage || 'Bilinmeyen baglanti hatasi.', {
    kind: 'network',
    endpoint,
    cause: err,
  });
}

function shouldRetry(error: ClientRequestError, attempt: number, retries: number): boolean {
  if (attempt >= retries) return false;
  return error.kind === 'network' || error.kind === 'timeout';
}

interface ParsedHttpError {
  message: string;
  apiCode?: string;
  retryable?: boolean;
}

async function parseHttpErrorMessage(res: Response): Promise<ParsedHttpError> {
  const data = await res.json().catch(() => null);
  if (data && typeof data === 'object') {
    const body = data as { code?: unknown; error?: unknown; retryable?: unknown };
    const err = body.error;
    const message =
      typeof err === 'string' && err.trim().length > 0 ? err.trim() : `HTTP ${res.status}`;
    if (res.status === 401) {
      return {
        message: 'Oturumun suresi doldu veya giris yapilmamis. Lutfen tekrar login ol.',
        apiCode: 'auth_unauthorized',
        retryable: false,
      };
    }
    return {
      message,
      apiCode: typeof body.code === 'string' && body.code.trim().length > 0 ? body.code.trim() : undefined,
      retryable: typeof body.retryable === 'boolean' ? body.retryable : undefined,
    };
  }
  if (res.status === 401) {
    return {
      message: 'Oturumun suresi doldu veya giris yapilmamis. Lutfen tekrar login ol.',
      apiCode: 'auth_unauthorized',
      retryable: false,
    };
  }
  return { message: `HTTP ${res.status}` };
}

export async function requestJson<T>(
  endpoint: string,
  init: RequestInit = {},
  options: RequestJsonOptions = {}
): Promise<T> {
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const retries = Math.max(0, options.retries ?? 0);
  const retryDelayMs = Math.max(0, options.retryDelayMs ?? DEFAULT_RETRY_DELAY_MS);

  for (let attempt = 0; ; attempt += 1) {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), timeoutMs);
    try {
      const res = await fetch(endpoint, {
        ...init,
        credentials: init.credentials ?? 'same-origin',
        signal: controller.signal,
      });
      clearTimeout(timeoutId);

      if (!res.ok) {
        const parsedError = await parseHttpErrorMessage(res);
        throw new ClientRequestError(parsedError.message, {
          kind: 'http',
          endpoint,
          status: res.status,
          apiCode: parsedError.apiCode,
          retryable: parsedError.retryable,
        });
      }

      if (res.status === 204) return {} as T;
      return (await res.json()) as T;
    } catch (err) {
      clearTimeout(timeoutId);
      const normalized = normalizeClientError(err, endpoint, timeoutMs);
      if (shouldRetry(normalized, attempt, retries)) {
        await sleep(retryDelayMs);
        continue;
      }
      throw normalized;
    }
  }
}

export function formatClientRequestError(error: unknown, fallback: string): string {
  if (error instanceof ClientRequestError) {
    if (error.kind === 'network') {
      return `${fallback} Sunucuya ulasilamadi. Endpoint: ${error.endpoint}`;
    }
    if (error.kind === 'timeout') {
      return `${fallback} Istek zaman asimina ugradi. Endpoint: ${error.endpoint}`;
    }
    if (error.kind === 'http') {
      if (error.status === 423) {
        return error.message;
      }
      if (error.status != null) return `${error.message} (HTTP ${error.status})`;
      return error.message;
    }
    return `${fallback} ${error.message}`.trim();
  }
  if (error instanceof Error) return error.message;
  return fallback;
}

export function hasClientRequestErrorCode(error: unknown, code: string): boolean {
  return error instanceof ClientRequestError && error.apiCode === code;
}
