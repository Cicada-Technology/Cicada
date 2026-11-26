/** 文件存储 API（`system:file:*`）。 */

import type { PageQuery, PageResult } from '@/api/types';
import { request } from '@/utils/request';

export interface FileVo {
  file_id: number;
  storage: string;
  original_name: string;
  size: number;
  mime: string;
  sha256: string;
  biz_type?: string | null;
  biz_id?: number | null;
  create_time: string;
}

export interface FileListQuery extends PageQuery {
  original_name?: string;
  biz_type?: string;
  sort_by?: string;
  sort_order?: string;
}

export interface DownloadUrlVo {
  url: string;
  kind: 'presigned' | 'one_time_token';
  expires_in: number;
}

export function listFiles(params: FileListQuery): Promise<PageResult<FileVo>> {
  return request.get<PageResult<FileVo>>('/api/v1/system/files', { params });
}

export function uploadFile(file: File): Promise<FileVo> {
  return request.upload<FileVo>('/api/v1/system/files', file);
}

export function deleteFile(fileId: number): Promise<void> {
  return request.delete<void>(`/api/v1/system/files/${fileId}`);
}

/** 直接下载（带 Bearer；适用于任何存储驱动）。 */
export function downloadFile(fileId: number, filename: string): Promise<void> {
  return request.download(`/api/v1/system/files/${fileId}/content`, { filename });
}

/** 签发短时链接：对象存储给预签名 URL，本地存储给一次性 token。 */
export function issueDownloadUrl(fileId: number): Promise<DownloadUrlVo> {
  return request.post<DownloadUrlVo>(`/api/v1/system/files/${fileId}/download-url`);
}
