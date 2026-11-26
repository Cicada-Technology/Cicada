use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

/// 补齐「接口与前端页面都已就绪、但旧库没有菜单行」的三组入口：
/// 文件管理、导出任务、变更留痕。
///
/// 种子（`seed.rs`）对全新库已经包含这些行；本迁移负责已有库升级。菜单与按钮一起
/// 补，并授权给默认租户管理员（role_id = 1），否则侧栏仍然看不到。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "INSERT INTO sys_menu \
                (menu_id, menu_name, parent_id, order_num, path, component, is_frame, is_cache, \
                 menu_type, visible, status, perms, icon, remark, create_by, create_time) \
             VALUES \
                (203, '文件管理', 1, 10, '/system/file', '', 1, 0, 'C', 0, 0, \
                 'system:file:view', 'FolderOpen', '文件存储', 'admin', now()), \
                (204, '导出任务', 1, 11, '/system/export', '', 1, 0, 'C', 0, 0, \
                 'system:export:view', 'Download', '异步导出', 'admin', now()), \
                (502, '变更留痕', 108, 3, '/monitor/auditlog', '', 1, 0, 'C', 0, 0, \
                 'monitor:auditlog:view', 'History', '字段级变更流水', 'admin', now()), \
                (1087, '文件查询', 203, 1, '#', '', 1, 0, 'F', 0, 0, \
                 'system:file:list', '', '', 'admin', now()), \
                (1088, '文件上传', 203, 2, '#', '', 1, 0, 'F', 0, 0, \
                 'system:file:add', '', '', 'admin', now()), \
                (1089, '文件删除', 203, 3, '#', '', 1, 0, 'F', 0, 0, \
                 'system:file:remove', '', '', 'admin', now()), \
                (1090, '任务查询', 204, 1, '#', '', 1, 0, 'F', 0, 0, \
                 'system:export:list', '', '', 'admin', now()), \
                (1091, '任务提交', 204, 2, '#', '', 1, 0, 'F', 0, 0, \
                 'system:export:add', '', '', 'admin', now()), \
                (1092, '留痕查询', 502, 1, '#', '', 1, 0, 'F', 0, 0, \
                 'monitor:auditlog:list', '', '', 'admin', now()) \
             ON CONFLICT (menu_id) DO NOTHING",
        )
        .await?;

        db.execute_unprepared(
            "INSERT INTO sys_role_menu (role_id, menu_id) \
             SELECT 1, menu_id FROM sys_menu \
              WHERE menu_id IN (203, 204, 502, 1087, 1088, 1089, 1090, 1091, 1092) \
                AND EXISTS (SELECT 1 FROM sys_role WHERE role_id = 1) \
             ON CONFLICT DO NOTHING",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "DELETE FROM sys_role_menu WHERE menu_id IN \
                (203, 204, 502, 1087, 1088, 1089, 1090, 1091, 1092)",
        )
        .await?;
        db.execute_unprepared(
            "DELETE FROM sys_menu WHERE menu_id IN \
                (203, 204, 502, 1087, 1088, 1089, 1090, 1091, 1092)",
        )
        .await?;
        Ok(())
    }
}
