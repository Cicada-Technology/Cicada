<script setup lang="ts">
/**
 * 异步导出任务（`system:export:*`）。
 *
 * 大表导出不走同步请求：任务落库后由后台执行，页面负责提交、看进度、拿产物。
 * 运行中的任务每 5 秒自动刷新一次，终态后停止轮询，避免空转。
 */
import { computed, onMounted, onUnmounted, reactive } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRoute } from 'vue-router';

import {
  createExportTask,
  downloadExportTask,
  EXPORT_MODULE_OPTIONS,
  EXPORT_STATUS_DONE,
  EXPORT_STATUS_FAILED,
  EXPORT_STATUS_OPTIONS,
  EXPORT_STATUS_RUNNING,
  listExportTasks,
  type ExportTaskListQuery,
  type ExportTaskVo,
} from '@/api/system/export';
import { STATUS_OPTIONS } from '@/api/types';
import AppDialog from '@/components/AppDialog.vue';
import AppDrawer from '@/components/AppDrawer.vue';
import AppPage from '@/components/AppPage.vue';
import AppSearchForm from '@/components/AppSearchForm.vue';
import AppTable from '@/components/AppTable.vue';
import { useTable } from '@/composables/useTable';
import { notifySuccess, notifyWarning } from '@/utils/feedback';
import { formatDateTime, orPlaceholder } from '@/utils/format';

const { t } = useI18n();
const route = useRoute();

const title = computed(() => route.meta.title ?? '导出任务');
const statusLabels = new Map(
  EXPORT_STATUS_OPTIONS.map((item) => [item.value as number, item.label]),
);

const {
  loading,
  rows,
  total,
  page,
  pageSize,
  query,
  load,
  search,
  reset,
  changePage,
  changePageSize,
} = useTable<ExportTaskVo, ExportTaskListQuery>({ fetcher: listExportTasks });

const hasRunning = computed(() =>
  rows.value.some((row) => row.status === 0 || row.status === EXPORT_STATUS_RUNNING),
);
let timer: number | undefined;
onMounted(() => {
  timer = window.setInterval(() => {
    if (hasRunning.value) void load();
  }, 5000);
});
onUnmounted(() => {
  if (timer !== undefined) window.clearInterval(timer);
});

const detail = reactive({ visible: false, row: null as ExportTaskVo | null });
const form = reactive({
  visible: false,
  module: 'system:user',
  user_name: '',
  status: undefined as number | undefined,
  submitting: false,
});

function openCreate(): void {
  form.module = 'system:user';
  form.user_name = '';
  form.status = undefined;
  form.visible = true;
}

async function submitCreate(): Promise<void> {
  if (form.module.trim() === '') {
    notifyWarning('请选择导出模块');
    return;
  }
  const filters: Record<string, unknown> = {};
  if (form.user_name.trim() !== '') filters.user_name = form.user_name.trim();
  if (form.status !== undefined) filters.status = form.status;
  form.submitting = true;
  try {
    const task = await createExportTask({ module: form.module, query: filters });
    form.visible = false;
    notifySuccess(`已受理任务 #${task.id}`);
    await load();
  } catch {
    // 提示由拦截器给出。
  } finally {
    form.submitting = false;
  }
}

async function download(row: ExportTaskVo): Promise<void> {
  if (row.status !== EXPORT_STATUS_DONE) {
    notifyWarning('任务尚未完成');
    return;
  }
  try {
    await downloadExportTask(row.id, `export-${row.module.replace(':', '-')}-${row.id}.xlsx`);
  } catch {
    // 提示由拦截器给出。
  }
}

function openDetail(row: ExportTaskVo): void {
  detail.row = row;
  detail.visible = true;
}

function statusType(status: number): 'success' | 'danger' | 'warning' | 'info' {
  if (status === EXPORT_STATUS_DONE) return 'success';
  if (status === EXPORT_STATUS_FAILED) return 'danger';
  if (status === EXPORT_STATUS_RUNNING) return 'warning';
  return 'info';
}
</script>

<template>
  <AppPage :title="title" :description="'大表导出走后台任务；完成后在列表里下载产物。'">
    <template #actions>
      <el-button v-perm="'system:export:add'" type="primary" @click="openCreate">
        新建导出
      </el-button>
    </template>

    <template #search>
      <AppSearchForm :model="query" :loading="loading" @search="search" @reset="reset">
        <el-form-item label="模块">
          <el-select v-model="query.module" clearable placeholder="全部" style="width: 150px">
            <el-option
              v-for="item in EXPORT_MODULE_OPTIONS"
              :key="item.value"
              :label="item.label"
              :value="item.value"
            />
          </el-select>
        </el-form-item>
        <el-form-item :label="t('common.status')">
          <el-select v-model="query.status" clearable placeholder="全部" style="width: 130px">
            <el-option
              v-for="item in EXPORT_STATUS_OPTIONS"
              :key="item.value"
              :label="item.label"
              :value="item.value"
            />
          </el-select>
        </el-form-item>
      </AppSearchForm>
    </template>

    <AppTable
      :rows="rows"
      :loading="loading"
      :total="total"
      :page="page"
      :page-size="pageSize"
      row-key="id"
      empty-title="还没有导出任务"
      empty-hint="点右上角「新建导出」；大表会在后台生成 xlsx。"
      @update:page="changePage"
      @update:page-size="changePageSize"
    >
      <el-table-column prop="id" label="#" width="80" />
      <el-table-column label="模块" min-width="150">
        <template #default="{ row }">{{ row.module }}</template>
      </el-table-column>
      <el-table-column label="状态" width="110">
        <template #default="{ row }">
          <el-tag :type="statusType(row.status)" size="small" effect="light" round>
            {{ statusLabels.get(row.status) ?? row.status }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="进度" width="130">
        <template #default="{ row }">{{ row.processed }} / {{ row.total || '—' }}</template>
      </el-table-column>
      <el-table-column label="错误" min-width="180">
        <template #default="{ row }">{{ orPlaceholder(row.error) }}</template>
      </el-table-column>
      <el-table-column label="完成时间" width="170">
        <template #default="{ row }">{{ formatDateTime(row.finish_time) }}</template>
      </el-table-column>
      <el-table-column label="创建时间" width="170">
        <template #default="{ row }">{{ formatDateTime(row.create_time) }}</template>
      </el-table-column>

      <el-table-column :label="t('common.actions')" width="160" fixed="right">
        <template #default="{ row }">
          <el-button
            v-perm="'system:export:list'"
            link
            type="primary"
            :disabled="row.status !== EXPORT_STATUS_DONE"
            @click="download(row)"
          >
            {{ t('common.download') }}
          </el-button>
          <el-button v-perm="'system:export:list'" link @click="openDetail(row)">
            {{ t('common.detail') }}
          </el-button>
        </template>
      </el-table-column>
    </AppTable>

    <AppDialog
      v-model="form.visible"
      title="新建导出任务"
      :confirm-loading="form.submitting"
      @confirm="submitCreate"
    >
      <el-form label-width="90px" @submit.prevent>
        <el-form-item label="模块" required>
          <el-select v-model="form.module" style="width: 100%">
            <el-option
              v-for="item in EXPORT_MODULE_OPTIONS"
              :key="item.value"
              :label="item.label"
              :value="item.value"
            />
          </el-select>
        </el-form-item>
        <el-form-item label="账号">
          <el-input v-model="form.user_name" clearable placeholder="用户导出可按登录名筛选" />
        </el-form-item>
        <el-form-item :label="t('common.status')">
          <el-select v-model="form.status" clearable placeholder="全部" style="width: 100%">
            <el-option
              v-for="item in STATUS_OPTIONS"
              :key="item.value"
              :label="item.value === 0 ? '正常' : '停用'"
              :value="item.value"
            />
          </el-select>
        </el-form-item>
      </el-form>
      <p class="export__hint">任务按当前账号的数据范围执行；完成后会出现在列表里。</p>
    </AppDialog>

    <AppDrawer
      v-model="detail.visible"
      :title="`任务 #${detail.row?.id ?? ''}`"
      size="520px"
    >
      <el-descriptions v-if="detail.row" :column="1" border>
        <el-descriptions-item label="模块">{{ detail.row.module }}</el-descriptions-item>
        <el-descriptions-item label="状态">
          {{ statusLabels.get(detail.row.status) ?? detail.row.status }}
        </el-descriptions-item>
        <el-descriptions-item label="进度">
          {{ detail.row.processed }} / {{ detail.row.total || '—' }}
        </el-descriptions-item>
        <el-descriptions-item label="错误">
          {{ orPlaceholder(detail.row.error) }}
        </el-descriptions-item>
      </el-descriptions>
      <div v-if="detail.row" class="export__query">
        <h3>条件快照</h3>
        <pre>{{ JSON.stringify(detail.row.query, null, 2) }}</pre>
      </div>
    </AppDrawer>
  </AppPage>
</template>

<style scoped>
.export__hint {
  margin-top: var(--ci-space-3);
  font-size: var(--ci-text-sm);
  color: var(--ci-color-text-tertiary);
}

.export__query {
  margin-top: var(--ci-space-5);
}

.export__query h3 {
  margin-bottom: var(--ci-space-2);
  font-size: var(--ci-text-sm);
  color: var(--ci-color-text-secondary);
}

.export__query pre {
  margin: 0;
  padding: var(--ci-space-3);
  overflow: auto;
  border-radius: var(--ci-radius-md);
  background: var(--ci-color-surface-sunken);
  font-family: var(--ci-font-mono);
  font-size: var(--ci-text-xs);
  color: var(--ci-color-text-secondary);
}
</style>
