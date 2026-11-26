<script setup lang="ts">
/**
 * 系统接口（`tool:swagger:view`）。
 *
 * Swagger UI 由后端提供，前端不重复实现文档渲染。生产同源时走相对路径；
 * 前后端不同源时由 `VITE_API_BASE_URL` 指向 API 源。
 */
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRoute } from 'vue-router';

import AppPage from '@/components/AppPage.vue';
import { openExternalUrl } from '@/utils/native';

const { t } = useI18n();
const route = useRoute();

const title = computed(() => route.meta.title ?? '系统接口');
const base = (import.meta.env.VITE_API_BASE_URL ?? '').replace(/\/+$/, '');
const docsUrl = `${base}/docs`;
const openapiUrl = `${base}/api-docs/openapi.json`;

function open(url: string): void {
  void openExternalUrl(url);
}
</script>

<template>
  <AppPage :title="title" :description="'运行中的 OpenAPI 3.1 文档；由后端直接提供。'">
    <div class="swagger ci-card">
      <h2>Swagger UI</h2>
      <p>浏览、调试全部 REST 接口；文档与实际路由同源，随产物一起交付。</p>
      <div class="swagger__actions">
        <el-button type="primary" @click="open(docsUrl)">
          {{ t('common.detail') }} /docs
        </el-button>
        <el-button @click="open(openapiUrl)">openapi.json</el-button>
      </div>
      <dl class="swagger__facts">
        <dt>文档地址</dt>
        <dd><code>{{ docsUrl }}</code></dd>
        <dt>OpenAPI</dt>
        <dd><code>{{ openapiUrl }}</code></dd>
      </dl>
    </div>
  </AppPage>
</template>

<style scoped>
.swagger {
  max-width: 760px;
  padding: var(--ci-space-6);
}

.swagger h2 {
  margin-bottom: var(--ci-space-2);
  font-size: var(--ci-text-xl);
  color: var(--ci-color-text);
}

.swagger p {
  margin-bottom: var(--ci-space-4);
  color: var(--ci-color-text-secondary);
  line-height: var(--ci-leading-relaxed);
}

.swagger__actions {
  display: flex;
  gap: var(--ci-space-2);
  margin-bottom: var(--ci-space-6);
}

.swagger__facts {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: var(--ci-space-2) var(--ci-space-4);
  font-size: var(--ci-text-sm);
}

.swagger__facts dt {
  color: var(--ci-color-text-tertiary);
}

.swagger__facts dd {
  margin: 0;
  min-width: 0;
  overflow-wrap: anywhere;
}

.swagger__facts code {
  font-family: var(--ci-font-mono);
  font-size: var(--ci-text-xs);
  color: var(--ci-color-text-secondary);
}
</style>
