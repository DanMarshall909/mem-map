import type { ChangeReading, Decimal, Reading, Row } from './contract.ts';

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
/** Presentation only: the backend supplies the coherent exact byte delta. */
export function changePercent(value: ChangeReading, current: Reading, lifecycle: Row['lifecycle'] = 'live'): { label: string; color: string; help: string } {
  const raw = `${change(value)}; ${value.value ?? 'Unavailable'} raw bytes${value.reason ? `; ${value.reason}` : ''}`;
  const unavailable = (reason: string) => ({ label: '—', color: '#b8c0c6', help: `${raw}; Percentage unavailable: ${reason}` });
  if (value.value === null) return unavailable('no complete baseline change');
  const delta = BigInt(value.value);
  const confirmedExit = lifecycle === 'exited' && delta <= 0n;
  if (!confirmedExit && (current.value === null || partial(current))) return unavailable('current memory measurement is incomplete');
  const now = confirmedExit ? 0n : BigInt(current.value!);
  const baseline = now - delta;
  if (now < 0n || baseline < 0n) return unavailable('inconsistent baseline');
  const help = `${raw}; Percentage relative to ${baseline} baseline bytes (${bytes(baseline.toString())})`;
  if (delta === 0n) return { label: '0.0%', color: '#ffffff', help };
  const positive = delta > 0n, magnitude = positive ? delta : -delta;
  const target = positive ? [255, 160, 160] : [135, 220, 170];
  const strength = baseline === 0n ? 1 : Math.sqrt(ratio(magnitude.toString(), baseline.toString()));
  const color = `rgb(${target.map(channel => Math.round(255 + (channel - 255) * strength)).join(', ')})`;
  if (baseline === 0n) return { label: 'New', color, help: `${raw}; Increased from zero; no finite percentage` };
  const tenths = (magnitude * 1000n + baseline / 2n) / baseline;
  const label = `${positive ? '+' : '−'}${tenths === 0n ? '<0.1' : `${tenths / 10n}.${tenths % 10n}`}%`;
  return { label, color, help };
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
