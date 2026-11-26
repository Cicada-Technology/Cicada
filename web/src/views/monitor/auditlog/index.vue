<script setup lang="ts">
/**
 * 字段级变更留痕（`monitor:auditlog:list`）。
 *
 * 只读视图：一条记录对应一次实体写操作，`changed` 里是
 * `{ field: [old, new] }`。详情抽屉保留原始 JSON，便于对账。
 */
import { computed, reactive } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRoute } from 'vue-router';

import {
  AUDIT_ACTION_OPTIONS,
  listAuditLogs,
  type AuditLogListQuery,
  type AuditLogVo,
} from '@/api/monitor/audit';
import AppDrawer from '@/components/AppDrawer.vue';
import AppPage from '@/components/AppPage.vue';
import AppSearchForm from '@/components/AppSearchForm.vue';
import AppTable from '@/components/AppTable.vue';
import { useTable } from '@/composables/useTable';
import { formatDateTime, orPlaceholder } from '@/utils/format';

const { t } = useI18n();
const route = useRoute();

const title = computed(() => route.meta.title ?? '变更留痕');
const actionLabels = new Map<string, string>(
  AUDIT_ACTION_OPTIONS.map((item) => [item.value, item.label]),
);

const {
  loading,
  rows,
  total,
  page,
  pageSize,
  query,
  search,
  reset,
  changePage,
  changePageSize,
} = useTable<AuditLogVo, AuditLogListQuery>({ fetcher: listAuditLogs });

const detail = reactive({ visible: false, row: null as AuditLogVo | null });

function openDetail(row: AuditLogVo): void {
  detail.row = row;
  detail.visible = true;
}

function actionLabel(action: string): string {
  return actionLabels.get(action) ?? action;
}

function prettyChanged(row: AuditLogVo | null): string {
  return row ? JSON.stringify(row.changed, null, 2) : '';
}
</script>

<template>
  <AppPage :title="title" :description="'实体级变更流水；每条记录带操作人与 trace，便于追溯。'">
    <template #search>
      <AppSearchForm :model="query" :loading="loading" @search="search" @reset="reset">
        <el-form-item label="表名">
          <el-input v-model="query.table_name" clearable placeholder="如 sys_user" />
        </el-form-item>
        <el-form-item label="记录 id">
          <el-input v-model="query.record_id" clearable placeholder="主键" />
        </el-form-item>
        <el-form-item label="动作">
          <el-select v-model="query.action" clearable placeholder="全部" style="width: 120px">
            <el-option
              v-for="item in AUDIT_ACTION_OPTIONS"
              :key="item.value"
              :label="item.label"
              :value="item.value"
            />
          </el-select>
        </el-form-item>
        <el-form-item label="开始">
          <el-date-picker
            v-model="query.begin_time"
            type="datetime"
            value-format="YYYY-MM-DDTHH:mm:ssZ"
            placeholder="开始时间"
            style="width: 190px"
          />
        </el-form-item>
        <el-form-item label="结束">
          <el-date-picker
            v-model="query.end_time"
            type="datetime"
            value-format="YYYY-MM-DDTHH:mm:ssZ"
            placeholder="结束时间"
            style="width: 190px"
          />
        </el-form-item>
      </AppSearchForm>
    </template>

    <AppTable
      :rows="rows"
      :loading="loading"
      :total="total"
      :page="page"
      :page-size="pageSize"
      row-key="audit_id"
      empty-title="没有变更记录"
      empty-hint="带字段级留痕的写操作会出现在这里。"
      @update:page="changePage"
      @update:page-size="changePageSize"
    >
      <el-table-column prop="audit_id" label="#" width="80" />
      <el-table-column prop="table_name" label="表" min-width="150" />
      <el-table-column prop="record_id" label="记录 id" min-width="120" />
      <el-table-column label="动作" width="100">
        <template #default="{ row }">
          <el-tag
            :type="row.action === 'delete' ? 'danger' : row.action === 'insert' ? 'success' : 'warning'"
            size="small"
            effect="light"
            round
          >
            {{ actionLabel(row.action) }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="操作人" width="100">
        <template #default="{ row }">{{ orPlaceholder(row.oper_id) }}</template>
      </el-table-column>
      <el-table-column label="trace" min-width="200" show-overflow-tooltip>
        <template #default="{ row }">{{ orPlaceholder(row.trace_id) }}</template>
      </el-table-column>
      <el-table-column label="时间" width="170">
        <template #default="{ row }">{{ formatDateTime(row.oper_time) }}</template>
      </el-table-column>

      <el-table-column :label="t('common.actions')" width="90" fixed="right">
        <template #default="{ row }">
          <el-button v-perm="'monitor:auditlog:list'" link type="primary" @click="openDetail(row)">
            {{ t('common.detail') }}
          </el-button>
        </template>
      </el-table-column>
    </AppTable>

    <AppDrawer
      v-model="detail.visible"
      :title="`${detail.row?.table_name ?? ''} #${detail.row?.record_id ?? ''}`"
      size="560px"
    >
      <el-descriptions v-if="detail.row" :column="1" border>
        <el-descriptions-item label="动作">
          {{ actionLabel(detail.row.action) }}
        </el-descriptions-item>
        <el-descriptions-item label="操作人 id">
          {{ orPlaceholder(detail.row.oper_id) }}
        </el-descriptions-item>
        <el-descriptions-item label="trace">{{ orPlaceholder(detail.row.trace_id) }}</el-descriptions-item>
        <el-descriptions-item label="时间">
          {{ formatDateTime(detail.row.oper_time) }}
        </el-descriptions-item>
      </el-descriptions>
      <div class="audit__changed">
        <h3>字段变化</h3>
        <pre>{{ prettyChanged(detail.row) }}</pre>
      </div>
    </AppDrawer>
  </AppPage>
</template>

<style scoped>
.audit__changed {
  margin-top: var(--ci-space-5);
}

.audit__changed h3 {
  margin-bottom: var(--ci-space-2);
  font-size: var(--ci-text-sm);
  color: var(--ci-color-text-secondary);
}

.audit__changed pre {
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
