import type { ChangeReading, Decimal, Reading } from './contract.ts';

export function bytes(value: Decimal | null): string {
  if (value === null) return '—';
  const n = BigInt(value);
  const magnitude = n < 0n ? -n : n;
  const [divisor, unit, digits] = magnitude >= 1073741824n ? [1073741824n, 'GiB', 2] as const : [1048576n, 'MiB', 1] as const;
  const factor = digits === 2 ? 100n : 10n;
  const rounded = (magnitude * factor + divisor / 2n) / divisor;
  return `${n < 0n && rounded !== 0n ? '−' : ''}${rounded / factor}.${(rounded % factor).toString().padStart(digits, '0')} ${unit}`;
}
export function change(value: ChangeReading): string {
  if (value.value === null) return '—';
  const label = bytes(value.value);
  return BigInt(value.value) > 0n && !/^0\.0 /.test(label) ? `+${label}` : label;
}
export function partial(r: Reading): boolean { return r.value !== null && (r.measured < r.total || !r.enumeration_complete); }
export function reading(r: Reading): string { return `${bytes(r.value)}${partial(r) ? ' · Partial' : ''}`; }
export function readingHelp(r: Reading): string {
  return `${r.value === null ? 'Unavailable' : `${r.value} bytes`}; ${r.measured} of ${r.total} processes measured${r.enumeration_complete ? '' : '; enumeration incomplete'}${r.reason ? `; ${r.reason}` : ''}`;
}
export function elapsed(seconds: number): string {
  const n = Math.max(0, Math.floor(seconds));
  return `${Math.floor(n / 60)}:${(n % 60).toString().padStart(2, '0')}`;
}
export function duration(seconds: number): string { return `${Math.floor(seconds / 60)}m ${(seconds % 60).toString().padStart(2, '0')}s`; }
export function rangeLabel(seconds: number | null): string { return seconds === null ? 'All retained history' : `${seconds / 60}m`; }
export function dateTime(unixMs: Decimal): string { return new Date(Number(unixMs)).toLocaleTimeString(); }
export function plotValue(value: Decimal | null): number | null { return value === null ? null : Number(value); }
export function ratio(value: Decimal | null, maximum: Decimal): number {
  return value === null || BigInt(maximum) <= 0n ? 0 : Math.min(1, Math.max(0, Number(value) / Number(maximum)));
}
