use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

/// 移除「表单构建」菜单：该能力是设计明确非目标，保留菜单会让用户永远落到占位页。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DELETE FROM sys_role_menu WHERE menu_id = 114")
            .await?;
        db.execute_unprepared("DELETE FROM sys_menu WHERE menu_id = 114")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "INSERT INTO sys_menu \
                (menu_id, menu_name, parent_id, order_num, path, component, is_frame, is_cache, \
                 menu_type, visible, status, perms, icon, create_by, create_time) \
             VALUES (114, '表单构建', 3, 1, '/tool/build', '', 1, 0, 'C', 0, 0, \
                     'tool:build:view', '', 'admin', now()) \
             ON CONFLICT (menu_id) DO NOTHING",
        )
        .await?;
        Ok(())
    }
}
