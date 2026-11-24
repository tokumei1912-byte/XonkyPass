import { errorMessages } from '../i18n/ru';

/** Ошибка команды: `{ code, message }` из Rust (или из mock-бэкенда). */
export class ApiError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = 'ApiError';
    this.code = code;
  }
}

/** Приводит отказ `invoke()` к `ApiError`. */
export function toApiError(error: unknown): ApiError {
  if (error instanceof ApiError) return error;
  if (typeof error === 'object' && error !== null && 'code' in error) {
    const { code, message } = error as { code: unknown; message?: unknown };
    return new ApiError(String(code), typeof message === 'string' ? message : '');
  }
  if (error instanceof Error) return new ApiError('internal', error.message);
  return new ApiError('internal', String(error));
}

/** Текст ошибки для пользователя. */
export function errorText(error: unknown): string {
  const { code, message } = toApiError(error);
  // У `invalid` Rust пишет, что именно не так (поле, ограничение) — это и показываем.
  if (code === 'invalid' && message) return capitalize(message);
  const text = errorMessages[code];
  if (text) return text;
  return message ? `${errorMessages.internal}: ${message}` : errorMessages.internal!;
}

function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
