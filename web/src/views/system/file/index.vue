<script setup lang="ts">
/**
 * 文件管理（`system:file:*`）。
 *
 * 文件元数据与内容分开：列表只读 `sys_file`，下载才拿内容。对象存储走
 * 「短时链接」，本地驱动走一次性 token；两条路径都由后端签发，前端不拼路径。
 */
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRoute } from 'vue-router';

import {
  deleteFile,
  downloadFile,
  issueDownloadUrl,
  listFiles,
  uploadFile,
  type FileListQuery,
  type FileVo,
} from '@/api/system/file';
import AppPage from '@/components/AppPage.vue';
import AppSearchForm from '@/components/AppSearchForm.vue';
import AppTable from '@/components/AppTable.vue';
import { useConfirm } from '@/composables/useConfirm';
import { useTable } from '@/composables/useTable';
import { copyText, pickFile } from '@/utils/download';
import { notifySuccess, notifyWarning } from '@/utils/feedback';
import { formatBytes, formatDateTime, orPlaceholder } from '@/utils/format';

const { t } = useI18n();
const route = useRoute();
const { confirm } = useConfirm();

const title = computed(() => route.meta.title ?? '文件管理');
const uploading = ref(false);

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
} = useTable<FileVo, FileListQuery>({ fetcher: listFiles });

async function upload(): Promise<void> {
  const file = await pickFile();
  if (!file) return;
  uploading.value = true;
  try {
    await uploadFile(file);
    notifySuccess(`已上传 ${file.name}`);
    await load();
  } catch {
    // 提示由 request 拦截器统一给出。
  } finally {
    uploading.value = false;
  }
}

async function download(row: FileVo): Promise<void> {
  try {
    await downloadFile(row.file_id, row.original_name);
  } catch {
    // 提示由拦截器给出。
  }
}

async function copyLink(row: FileVo): Promise<void> {
  try {
    const grant = await issueDownloadUrl(row.file_id);
    const ok = await copyText(grant.url);
    if (ok) notifySuccess(`下载链接已复制，${grant.expires_in} 秒内有效`);
    else notifyWarning('复制失败，请改用下载按钮');
  } catch {
    // 提示由拦截器给出。
  }
}

async function remove(row: FileVo): Promise<void> {
  const agreed = await confirm(`确定删除「${row.original_name}」吗？文件内容会一并删除。`, {
    danger: true,
  });
  if (!agreed) return;
  try {
    await deleteFile(row.file_id);
    notifySuccess('已删除');
    await load();
  } catch {
    // 提示由拦截器给出。
  }
}
</script>

<template>
  <AppPage :title="title" :description="'租户内的文件元数据；上传、下载与删除都在这里闭环。'">
    <template #actions>
      <el-button v-perm="'system:file:add'" type="primary" :loading="uploading" @click="upload">
        上传文件
      </el-button>
    </template>

    <template #search>
      <AppSearchForm :model="query" :loading="loading" @search="search" @reset="reset">
        <el-form-item label="文件名">
          <el-input v-model="query.original_name" clearable placeholder="模糊搜索" />
        </el-form-item>
        <el-form-item label="业务类型">
          <el-input v-model="query.biz_type" clearable placeholder="如 avatar" />
        </el-form-item>
      </AppSearchForm>
    </template>

    <AppTable
      :rows="rows"
      :loading="loading"
      :total="total"
      :page="page"
      :page-size="pageSize"
      row-key="file_id"
      empty-title="还没有文件"
      empty-hint="点右上角「上传文件」开始；文件会按租户隔离。"
      @update:page="changePage"
      @update:page-size="changePageSize"
    >
      <el-table-column label="文件名" min-width="240">
        <template #default="{ row }">
          <span class="file__name">{{ row.original_name }}</span>
        </template>
      </el-table-column>
      <el-table-column label="大小" width="120">
        <template #default="{ row }">{{ formatBytes(row.size) }}</template>
      </el-table-column>
      <el-table-column prop="mime" label="MIME" min-width="170" show-overflow-tooltip />
      <el-table-column prop="storage" label="存储" width="110" />
      <el-table-column label="业务类型" min-width="130">
        <template #default="{ row }">{{ orPlaceholder(row.biz_type) }}</template>
      </el-table-column>
      <el-table-column label="上传时间" width="170">
        <template #default="{ row }">{{ formatDateTime(row.create_time) }}</template>
      </el-table-column>

      <el-table-column :label="t('common.actions')" width="210" fixed="right">
        <template #default="{ row }">
          <el-button v-perm="'system:file:list'" link type="primary" @click="download(row)">
            下载
          </el-button>
          <el-button v-perm="'system:file:list'" link @click="copyLink(row)">复制链接</el-button>
          <el-button v-perm="'system:file:remove'" link type="danger" @click="remove(row)">
            {{ t('common.delete') }}
          </el-button>
        </template>
      </el-table-column>
    </AppTable>
  </AppPage>
</template>

<style scoped>
.file__name {
  font-weight: var(--ci-weight-medium);
  color: var(--ci-color-text);
  word-break: break-all;
}
</style>
