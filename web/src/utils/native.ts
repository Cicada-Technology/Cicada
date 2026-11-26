/**
 * Tauri 原生壳适配层（桌面 + Android/iOS）。
 *
 * 三件事：把浏览器 localStorage 的令牌持久化换成平台安全存储、用系统浏览器完成
 * OAuth 并把 `cicada://` 回调接回应用、下载落盘。业务代码仍然只认 `utils/session.ts`
 * 与普通 API，不感知运行环境；REST 协议、路由与页面完全复用。
 */
import type { Router } from 'vue-router';

export interface NativeTokens {
  accessToken: string;
  refreshToken: string;
}

/** Tauri 2 在 webview 里注入的内部对象；普通浏览器和移动端浏览器都没有。 */
export function isTauriApp(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: call } = (await import('@tauri-apps/api/core')) as { invoke: Invoke };
  return call<T>(command, args);
}

export async function loadNativeTokens(): Promise<NativeTokens | null> {
  if (!isTauriApp()) return null;
  try {
    const raw = await invoke<string | null>('secure_load');
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<NativeTokens>;
    if (typeof parsed.accessToken !== 'string' || typeof parsed.refreshToken !== 'string') {
      return null;
    }
    return { accessToken: parsed.accessToken, refreshToken: parsed.refreshToken };
  } catch {
    // 平台安全存储不可用时退回 localStorage；原生端不把它当错误抛给用户。
    return null;
  }
}

export async function saveNativeTokens(tokens: NativeTokens): Promise<boolean> {
  if (!isTauriApp()) return true;
  try {
    await invoke('secure_save', { payload: JSON.stringify(tokens) });
    return true;
  } catch {
    // 持久化失败不应阻塞登录；调用方会降级到 localStorage。
    return false;
  }
}

export async function clearNativeTokens(): Promise<void> {
  if (!isTauriApp()) return;
  try {
    await invoke('secure_clear');
  } catch {
    // 同上。
  }
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

/** 原生端把下载字节写入平台可写目录；浏览器返回 null 由调用方走 anchor。 */
export async function saveNativeFile(filename: string, blob: Blob): Promise<string | null> {
  if (!isTauriApp()) return null;
  const bytes = new Uint8Array(await blob.arrayBuffer());
  return invoke<string>('save_file', {
    filename,
    dataBase64: bytesToBase64(bytes),
  });
}

/**
 * 在系统浏览器打开外部链接。
 *
 * Tauri webview 不保证实现 `window.open` 的跨源新窗口，外链菜单与 Swagger 统一走这里；
 * 浏览器端保持新标签页语义。
 */
export async function openExternalUrl(url: string): Promise<void> {
  if (!isTauriApp()) {
    window.open(url, '_blank', 'noopener,noreferrer');
    return;
  }
  await invoke('open_external', { url });
}

/**
 * 打开第三方登录授权页。
 *
 * 原生端（桌面/Android/iOS）必须交给系统浏览器：嵌入 webview 的第三方登录既可能被 IdP 拒绝，
 * 也无法把 `cicada://` 回调交还给应用。浏览器端保持原跳转。
 */
export async function openOAuthAuthorizeUrl(url: string): Promise<void> {
  if (!isTauriApp()) {
    window.location.assign(url);
    return;
  }
  await invoke('open_external', { url });
}

/**
 * 安装原生端 OAuth 回调监听。
 *
 * 两种到达方式都要接住：
 * - 应用已在运行：Rust 侧 `on_open_url` 发 `cicada://oauth-url` 事件；
 * - 应用被 deep link 冷启动：启动参数被缓存，挂载前用 `oauth_pending_url` 取走。
 * 同一 URL 可能两条路都到，按原文去重。
 */
export async function initNativeOAuth(router: Router): Promise<void> {
  if (!isTauriApp()) return;
  let lastHandled: string | null = null;
  const handle = (raw: string): void => {
    if (raw === lastHandled) return;
    lastHandled = raw;
    let parsed: URL;
    try {
      parsed = new URL(raw);
    } catch {
      return;
    }
    if (parsed.protocol !== 'cicada:') return;
    const route = `${parsed.host}${parsed.pathname}`.replace(/^\/+|\/+$/g, '');
    if (route !== 'oauth/callback') return;
    const code = parsed.searchParams.get('code') ?? '';
    const state = parsed.searchParams.get('state') ?? '';
    if (code === '' || state === '') return;
    void router.replace({ path: '/oauth/callback', query: { code, state } });
  };

  try {
    const { listen } = await import('@tauri-apps/api/event');
    await listen<string>('cicada://oauth-url', (event) => handle(event.payload));
  } catch {
    // 事件通道不可用（权限或旧壳）不应阻塞应用启动；冷启动缓存仍会尝试。
  }
  const pending = await invoke<string | null>('oauth_pending_url').catch(() => null);
  if (pending) handle(pending);
}
