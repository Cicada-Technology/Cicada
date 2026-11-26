/** 第三方登录（OAuth2 / OIDC）。与密码登录共用同一套令牌与 `/auth/profile`。 */

import type { LoginResult } from '@/api/auth';
import { request } from '@/utils/request';

export interface OAuthProviderVo {
  code: string;
  kind: string;
}

export interface AuthorizeUrlVo {
  authorize_url: string;
  state: string;
}

export interface OAuthExternalVo {
  identity_type: string;
  provider: string;
  identifier: string;
  nickname?: string | null;
  avatar?: string | null;
  email?: string | null;
}

export interface OAuthCallbackVo {
  status: 'bound' | 'unbound';
  login?: LoginResult;
  binding_ticket?: string;
  external: OAuthExternalVo;
}

export interface BindOAuthVo {
  identity_id: number;
  identity_type: string;
  provider?: string | null;
  identifier: string;
}

export function listOAuthProviders(): Promise<OAuthProviderVo[]> {
  return request.get<OAuthProviderVo[]>('/auth/oauth/providers', { silent: true });
}

export function getOAuthAuthorizeUrl(
  provider: string,
  client?: 'native',
): Promise<AuthorizeUrlVo> {
  const suffix = client === 'native' ? '?client=native' : '';
  return request.get<AuthorizeUrlVo>(`/auth/login/${provider}/authorize-url${suffix}`, {
    silent: true,
  });
}

export function oauthCallback(
  provider: string,
  code: string,
  state: string,
  client?: 'native',
): Promise<OAuthCallbackVo> {
  return request.post<OAuthCallbackVo>(
    `/auth/login/${provider}/callback`,
    { code, state, client },
    { silent: true },
  );
}

export function bindOAuthIdentity(bindingTicket: string): Promise<BindOAuthVo> {
  return request.post<BindOAuthVo>('/auth/identities/oauth', {
    binding_ticket: bindingTicket,
  });
}
