<script setup lang="ts">
/**
 * 第三方登录回调。
 *
 * IdP 把浏览器重定向到这里；前端只负责把 code/state POST 回后端。
 * - 已绑定：直接拿令牌并进入应用；
 * - 未绑定：保存短时绑定票据，转登录页，用户用密码登录后自动完成绑定。
 */
import { onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';

import { oauthCallback } from '@/api/oauth';
import { useAuthStore } from '@/stores/auth';
import { usePermissionStore } from '@/stores/permission';
import { isTauriApp } from '@/utils/native';

const OAUTH_PROVIDER_KEY = 'cicada.oauth.provider';
const OAUTH_BINDING_KEY = 'cicada.oauth.binding';

const route = useRoute();
const router = useRouter();
const auth = useAuthStore();
const permission = usePermissionStore();

const message = ref('正在完成登录…');
const error = ref('');

function text(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

onMounted(async () => {
  const code = text(route.query.code);
  const state = text(route.query.state);
  let provider = '';
  try {
    provider = localStorage.getItem(OAUTH_PROVIDER_KEY) ?? '';
  } catch {
    // 隐私模式下无 localStorage；provider 丢失就无法完成回调。
  }
  if (code === '' || state === '' || provider === '') {
    error.value = '回调缺少 code / state / provider，请重新发起登录。';
    return;
  }
  try {
    const client = isTauriApp() ? 'native' : undefined;
    const result = await oauthCallback(provider, code, state, client);
    try {
      localStorage.removeItem(OAUTH_PROVIDER_KEY);
    } catch {
      // ignore
    }
    if (result.status === 'bound' && result.login) {
      auth.acceptTokens({
        accessToken: result.login.access_token,
        refreshToken: result.login.refresh_token,
      });
      await auth.loadProfile();
      permission.reset();
      await router.replace('/');
      return;
    }
    try {
      sessionStorage.setItem(OAUTH_BINDING_KEY, JSON.stringify(result));
    } catch {
      // 无 sessionStorage 时退回手工登录提示。
    }
    await router.replace({ path: '/login', query: { bind: '1' } });
  } catch (e) {
    error.value = e instanceof Error ? e.message : '第三方登录失败，请重试。';
  }
});
</script>

<template>
  <div class="oauth-callback">
    <div class="oauth-callback__card ci-card">
      <template v-if="error === ''">
        <p class="oauth-callback__title">{{ message }}</p>
        <p class="oauth-callback__hint">请保持窗口打开，完成后会自动进入管理端。</p>
      </template>
      <template v-else>
        <p class="oauth-callback__title">登录未完成</p>
        <p class="oauth-callback__error">{{ error }}</p>
        <el-button type="primary" @click="router.replace('/login')">返回登录</el-button>
      </template>
    </div>
  </div>
</template>

<style scoped>
.oauth-callback {
  display: grid;
  place-items: center;
  min-height: 100vh;
  min-height: 100dvh;
  background: var(--ci-color-bg);
}

.oauth-callback__card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--ci-space-2);
  max-width: 420px;
  padding: var(--ci-space-8);
  text-align: center;
}

.oauth-callback__title {
  font-size: var(--ci-text-lg);
  font-weight: var(--ci-weight-semibold);
  color: var(--ci-color-text);
}

.oauth-callback__hint {
  font-size: var(--ci-text-sm);
  color: var(--ci-color-text-tertiary);
}

.oauth-callback__error {
  font-size: var(--ci-text-sm);
  color: var(--ci-color-danger);
}
</style>
