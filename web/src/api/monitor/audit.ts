/** 字段级变更留痕（只读，`monitor:auditlog:list`）。 */

import type { PageQuery, PageResult } from '@/api/types';
import { request } from '@/utils/request';

export const AUDIT_ACTION_OPTIONS = [
  { value: 'insert', label: '新增' },
  { value: 'update', label: '修改' },
  { value: 'delete', label: '删除' },
] as const;

export interface AuditLogVo {
  audit_id: number;
  table_name: string;
  record_id: string;
  action: string;
  /** `{ field: [old, new] }`，删除时 new 为 null。 */
  changed: Record<string, [unknown, unknown]>;
  oper_id?: number | null;
  trace_id?: string | null;
  oper_time: string;
}

export interface AuditLogListQuery extends PageQuery {
  table_name?: string;
  record_id?: string;
  action?: string;
  begin_time?: string;
  end_time?: string;
}

export function listAuditLogs(params: AuditLogListQuery): Promise<PageResult<AuditLogVo>> {
  return request.get<PageResult<AuditLogVo>>('/api/v1/monitor/audit-logs', { params });
}
