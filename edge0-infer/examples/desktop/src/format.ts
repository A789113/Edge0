import { round } from './chat';
export const present = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value) && value >= 0;
export const count = (value?: number | null): string | null => present(value) ? Math.trunc(value).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',') : null;
export const bytes = (value?: number | null): string | null => present(value) ? value < 10_000_000 ? `${round(value / 1_000_000, 0)} MB` : `${round(value / 1_000_000_000, 2)} GB` : null;
export const rate = (value?: number | null): string | null => present(value) ? `${round(value, 1)} tok/s` : null;
// Half-up integer ratio of whole counts. Floating division would round 23 / 80
// (28.75%) down to 28.7%; the shared vectors and the mobile Apps use exact arithmetic.
const ratio = (numerator: number, denominator: number, scale: number): bigint => {
  const d = BigInt(Math.trunc(denominator));
  return (BigInt(Math.trunc(numerator)) * BigInt(scale) * 2n + d) / (2n * d);
};
export const percent = (numerator?: number | null, denominator?: number | null): string | null => {
  if (!present(numerator) || !present(denominator) || Math.trunc(denominator) <= 0) return null;
  const tenths = ratio(numerator, denominator, 1000);
  return `${tenths / 10n}.${tenths % 10n}%`;
};
export function duration(value?: number | null): string | null {
  if (!present(value)) return null;
  if (Number(round(value, 1)) < 60) return `${round(value, 1)} 秒`;
  const seconds = Number(round(value, 0));
  return seconds < 3600 ? `${Math.floor(seconds / 60)} 分 ${seconds % 60} 秒` : `${Math.floor(seconds / 3600)} 小时 ${Math.floor(seconds % 3600 / 60)} 分`;
}
export interface DeviceMemory { current_bytes: number; peak_bytes: number; memory: 'system' | 'dedicated'; }
export function deviceMemory(devices?: Record<string, DeviceMemory>) {
  if (!devices || !Object.keys(devices).length) return null;
  const values = Object.values(devices);
  return { system: bytes(values.filter(v => v.memory === 'system').reduce((sum, v) => sum + v.current_bytes, 0)), dedicated: values.some(v => v.memory === 'dedicated') ? bytes(values.filter(v => v.memory === 'dedicated').reduce((sum, v) => sum + v.current_bytes, 0)) : null };
}
export function contextUsage(input: { input_tokens?: number; generated_tokens?: number; context_length?: number }) {
  const used = (present(input.input_tokens) ? input.input_tokens : 0) + (present(input.generated_tokens) ? input.generated_tokens : 0);
  const capacity = present(input.context_length) && input.context_length > 0 ? input.context_length : undefined;
  return { text: capacity ? `${count(used)} / ${count(capacity)} 个 token` : `${count(used)} 个 token`, ring_percent: capacity ? Math.min(100, Number(ratio(used, capacity, 100))) : null };
}
export const time = (value?: string) => value && !Number.isNaN(Date.parse(value)) ? new Date(value).toLocaleTimeString('zh-CN', { hour12: false }) : null;
export type ThermalDisplay = { label: string; description: string; tone: 'ok' | 'warn' | 'error' };
const thermalLevels: [string, string, ThermalDisplay['tone'], string[]][] = [
  ['正常', '没有因发热降速', 'ok', ['nominal', 'none']],
  ['偏热', '降速很轻或没有', 'warn', ['fair', 'light']],
  ['已降速', '正减速散热', 'warn', ['serious', 'moderate', 'severe']],
  ['过热', '严重降速，需要散热', 'error', ['critical']],
  ['即将关机', '系统准备关闭部分组件', 'error', ['emergency', 'shutdown']],
];
export function thermal(state: string): ThermalDisplay {
  const level = thermalLevels.find(([, , , states]) => states.includes(state));
  return level ? { label: level[0], description: level[1], tone: level[2] } : { label: state, description: '未知状态', tone: 'warn' };
}
