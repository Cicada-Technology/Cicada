/** 认证接口。路由在 `/auth/*` 下，不在 `/api/v1` 命名空间里。 */

import { request } from '@/utils/request';

export interface LoginRequest {
  username: string;
  password: string;
  /** 目标租户 id；缺省使用后端 `tenant.default_id`。 */
  tenant_id?: number;
  /** 启用 TOTP 的账号需要带上 6 位动态码。 */
  totp?: string;
  /** 连续登录失败后必带，来自 `getCaptcha()`。 */
  captcha_id?: string;
  captcha_code?: string;
}

/** 图形验证码。`enabled=false` 时其余字段为空串。 */
export interface CaptchaResult {
  captcha_id: string;
  /** 不带 data URI 前缀的 base64 PNG。 */
  img: string;
  enabled: boolean;
}

export interface LoginResult {
  access_token: string;
  refresh_token: string;
  user_id: number;
  tenant_id: number;
  username: string;
}

export interface LoginTenantVo {
  tenant_id: number;
  tenant_name: string;
}

export interface ProfileResult {
  user_id: number;
  user_name: string;
  nick_name: string;
  email?: string | null;
  phonenumber?: string | null;
  tenant_id: number;
  /** 角色标识集合（`role_key`），管理员为 `admin`（超级管理员短路）。 */
  roles: string[];
  /** 权限标识集合，与菜单里的 `perms` 同名。 */
  permissions: string[];
}

export interface UpdateProfileRequest {
  nick_name: string;
  email?: string | null;
  phonenumber?: string | null;
}

export interface ChangePasswordRequest {
  old_password: string;
  new_password: string;
}

export interface IdentityVo {
  identity_id: number;
  identity_type: string;
  identifier: string;
  provider?: string | null;
  is_primary: boolean;
  verified: boolean;
}

export interface BindIdentityRequest {
  identity_type: string;
  identifier: string;
  provider?: string | null;
  code?: string | null;
}

export interface TotpStatus {
  enabled: boolean;
}

export interface TotpSetupResult {
  secret: string;
  otpauth_url: string;
}

/** `/auth/routers` 下发的菜单树节点（只含 `M` 目录与 `C` 菜单，按钮不进树）。 */
export interface MenuNode {
  menu_id: number;
  parent_id: number;
  menu_name: string;
  menu_type: string;
  path?: string | null;
  component?: string | null;
  perms?: string | null;
  order_num: number;
  children: MenuNode[];
}

/**
 * 登录。
 *
 * `silent: true`：登录失败要在表单里内联提示（账号不存在 / 密码错 / 已锁定 / 需要动态码
 * 是四种不同的处理路径），全局 toast 会盖住表单上的字段级反馈。
 */
export function login(data: LoginRequest): Promise<LoginResult> {
  return request.post<LoginResult>('/auth/login', data, { silent: true });
}

/** 启用中的租户列表；登录页据此提供租户选择。 */
export function getLoginTenants(): Promise<LoginTenantVo[]> {
  return request.get<LoginTenantVo[]>('/auth/tenants', { silent: true });
}

/** 取一张图形验证码；公开接口，登录页在收到「图形验证码」错误后调用并展示。 */
export function getCaptcha(): Promise<CaptchaResult> {
  return request.get<CaptchaResult>('/auth/captcha', { silent: true });
}

export function logout(): Promise<void> {
  return request.post<void>('/auth/logout', undefined, { silent: true });
}

export function getProfile(): Promise<ProfileResult> {
  return request.get<ProfileResult>('/auth/profile');
}

/** 当前用户可见的菜单树：动态路由与侧栏的唯一来源。 */
export function getRouters(): Promise<MenuNode[]> {
  return request.get<MenuNode[]>('/auth/routers');
}

export function updateProfile(data: UpdateProfileRequest): Promise<void> {
  return request.put<void>('/auth/profile', data);
}

export function changePassword(data: ChangePasswordRequest): Promise<void> {
  return request.put<void>('/auth/password', data);
}

export function listIdentities(): Promise<IdentityVo[]> {
  return request.get<IdentityVo[]>('/auth/identities');
}

export function bindIdentity(data: BindIdentityRequest): Promise<IdentityVo> {
  return request.post<IdentityVo>('/auth/identities', data);
}

export function unbindIdentity(identityId: number): Promise<void> {
  return request.delete<void>(`/auth/identities/${identityId}`);
}

export function getTotpStatus(): Promise<TotpStatus> {
  return request.get<TotpStatus>('/auth/totp');
}

export function setupTotp(): Promise<TotpSetupResult> {
  return request.post<TotpSetupResult>('/auth/totp/setup');
}

export function enableTotp(code: string): Promise<void> {
  return request.post<void>('/auth/totp/enable', { code });
}

export function disableTotp(code: string): Promise<void> {
  return request.post<void>('/auth/totp/disable', { code });
}
