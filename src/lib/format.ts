// Форматирование для интерфейса: числа, даты, надёжность пароля.

import type { Strength } from './api';

/** Русское склонение: `plural(3, 'запись', 'записи', 'записей')` → «записи». */
export function plural(n: number, one: string, few: string, many: string): string {
  const mod10 = n % 10;
  const mod100 = n % 100;
  if (mod10 === 1 && mod100 !== 11) return one;
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return few;
  return many;
}

export type Tone = 'danger' | 'warn' | 'ok';

export const STRENGTH_LABELS: readonly { label: string; tone: Tone }[] = [
  { label: 'очень слабый', tone: 'danger' },
  { label: 'слабый', tone: 'danger' },
  { label: 'средний', tone: 'warn' },
  { label: 'хороший', tone: 'ok' },
  { label: 'надёжный', tone: 'ok' },
];

export function strengthLabel(strength: Strength): { label: string; tone: Tone } {
  return STRENGTH_LABELS[strength.score] ?? STRENGTH_LABELS[0]!;
}

/**
 * Сколько займёт подбор, если украдут быстрый хеш пароля: 10¹⁰ попыток в секунду.
 * Для мастер-пароля это с большим запасом — его защищает медленный Argon2id.
 */
export function crackTime(guessesLog10: number): string {
  const seconds = guessesLog10 - 10;
  if (seconds < 0) return 'подберут мгновенно';
  const s = 10 ** seconds;
  if (s < 60) return 'подбор займёт секунды';
  if (s < 3600) return 'подбор займёт минуты';
  if (s < 86_400) return 'подбор займёт часы';
  if (s < 2_592_000) return 'подбор займёт дни';
  if (s < 31_536_000) return 'подбор займёт месяцы';
  if (s < 3_153_600_000) return 'подбор займёт годы';
  return 'подбор займёт века';
}

const DATE = new Intl.DateTimeFormat('ru', { day: 'numeric', month: 'short', year: 'numeric' });
const DATE_NO_YEAR = new Intl.DateTimeFormat('ru', { day: 'numeric', month: 'short' });
const TIME = new Intl.DateTimeFormat('ru', { hour: '2-digit', minute: '2-digit' });

/** «12 нояб. 2026». */
export function formatDate(ms: number): string {
  return DATE.format(ms).replace(/\s*г\.$/, '');
}

/** «только что», «5 минут назад», «вчера в 21:40», «3 дня назад», «12 нояб.». */
export function formatRelative(ms: number, now = Date.now()): string {
  const minutes = Math.floor((now - ms) / 60_000);
  if (minutes < 1) return 'только что';
  if (minutes < 60) return `${minutes} ${plural(minutes, 'минуту', 'минуты', 'минут')} назад`;

  const date = new Date(ms);
  const today = new Date(now);
  today.setHours(0, 0, 0, 0);
  const days = Math.floor((today.getTime() - startOfDay(date)) / 86_400_000);
  if (days <= 0) return `сегодня в ${TIME.format(ms)}`;
  if (days === 1) return `вчера в ${TIME.format(ms)}`;
  if (days < 7) return `${days} ${plural(days, 'день', 'дня', 'дней')} назад`;
  if (date.getFullYear() === today.getFullYear()) return DATE_NO_YEAR.format(ms);
  return formatDate(ms);
}

function startOfDay(date: Date): number {
  const copy = new Date(date);
  copy.setHours(0, 0, 0, 0);
  return copy.getTime();
}
