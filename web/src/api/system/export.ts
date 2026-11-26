/** 异步导出任务 API（`system:export:*`）。 */

import type { PageQuery, PageResult } from '@/api/types';
import { request } from '@/utils/request';

export const EXPORT_STATUS_PENDING = 0;
export const EXPORT_STATUS_RUNNING = 1;
export const EXPORT_STATUS_DONE = 2;
export const EXPORT_STATUS_FAILED = 3;

export const EXPORT_STATUS_OPTIONS = [
  { value: EXPORT_STATUS_PENDING, label: '等待中' },
  { value: EXPORT_STATUS_RUNNING, label: '执行中' },
  { value: EXPORT_STATUS_DONE, label: '已完成' },
  { value: EXPORT_STATUS_FAILED, label: '失败' },
] as const;

/** 后端当前注册的导出执行器（`crates/web/src/async_export.rs`）。 */
export const EXPORT_MODULE_OPTIONS = [{ value: 'system:user', label: '用户数据' }] as const;

export interface ExportTaskVo {
  id: number;
  module: string;
  query: Record<string, unknown>;
  status: number;
  file_id?: number | null;
  total: number;
  processed: number;
  error?: string | null;
  finish_time?: string | null;
  create_time: string;
}

export interface ExportTaskListQuery extends PageQuery {
  module?: string;
  status?: number;
  sort_by?: string;
  sort_order?: string;
}

export interface ExportRequest {
  module: string;
  query?: Record<string, unknown>;
}

export function listExportTasks(params: ExportTaskListQuery): Promise<PageResult<ExportTaskVo>> {
  return request.get<PageResult<ExportTaskVo>>('/api/v1/system/export-tasks', { params });
}

export function getExportTask(id: number): Promise<ExportTaskVo> {
  return request.get<ExportTaskVo>(`/api/v1/system/export-tasks/${id}`);
}

export function createExportTask(data: ExportRequest): Promise<ExportTaskVo> {
  return request.post<ExportTaskVo>('/api/v1/system/export-tasks', data);
}

export function downloadExportTask(id: number, filename?: string): Promise<void> {
  return request.download(`/api/v1/system/export-tasks/${id}/download`, { filename });
}
