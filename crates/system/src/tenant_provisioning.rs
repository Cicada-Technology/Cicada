//! 租户开通：在**一个事务**里建租户、管理员角色/账号/身份与全菜单授权。
//!
//! 这是「租户管理」与 CLI `tenant init` 共用的唯一实现。少一个步骤，租户就
//! 无法登录或看不到任何菜单，所以不允许调用方自己拼装。

use argon2::{Argon2, PasswordHasher};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use thiserror::Error;

use crate::entity::{
    MenuEntity, RoleActiveModel, RoleMenuActiveModel, TenantActiveModel, TenantModel,
    UserActiveModel, UserColumn, UserEntity, UserIdentityActiveModel, UserRoleActiveModel,
};

/// 开通租户的输入。
#[derive(Debug, Clone)]
pub struct NewTenant<'a> {
    pub name: &'a str,
    pub status: i16,
    pub package_id: Option<i64>,
    pub remark: Option<&'a str>,
    pub admin_username: &'a str,
    pub admin_password: &'a str,
    pub password_min_length: usize,
    pub create_by: Option<&'a str>,
}

/// 开通结果。
#[derive(Debug, Clone)]
pub struct ProvisionedTenant {
    pub tenant: TenantModel,
    pub admin_user_id: i64,
}

#[derive(Debug, Error)]
pub enum ProvisionError {
    #[error("数据库错误：{0}")]
    Db(#[from] DbErr),
    #[error("密码不合规：{0}")]
    Password(String),
    #[error("管理员账号不能为空")]
    EmptyUsername,
    #[error("管理员账号已存在：{0}")]
    DuplicateUsername(String),
}

/// 在事务里创建租户及其初始管理员。
///
/// 管理员账号统一小写：登录侧的 `canonical_identifier` 会小写化，保存原样会让
/// 「建得出来、登不进去」成为隐藏缺陷。
pub async fn create_tenant_with_admin(
    db: &DatabaseConnection,
    input: NewTenant<'_>,
) -> Result<ProvisionedTenant, ProvisionError> {
    let name = input.name.trim();
    let username = input.admin_username.trim().to_lowercase();
    if username.is_empty() {
        return Err(ProvisionError::EmptyUsername);
    }
    let problems = cicada_infra::password_problems(input.admin_password, input.password_min_length);
    if !problems.is_empty() {
        return Err(ProvisionError::Password(
            cicada_infra::password_problem_text(&problems, input.password_min_length),
        ));
    }
    let password_hash = Argon2::default()
        .hash_password(input.admin_password.as_bytes())
        .map_err(|e| ProvisionError::Db(DbErr::Custom(e.to_string())))?
        .to_string();

    let txn = db.begin().await?;
    let now = Utc::now();
    let tenant = TenantActiveModel {
        tenant_name: Set(name.to_string()),
        status: Set(input.status),
        package_id: Set(input.package_id),
        remark: Set(input.remark.map(str::to_string)),
        create_by: Set(input.create_by.map(str::to_string)),
        create_time: Set(now),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    let existing = UserEntity::find()
        .filter(UserColumn::TenantId.eq(tenant.tenant_id))
        .filter(UserColumn::UserName.eq(&username))
        .filter(UserColumn::DeletedAt.is_null())
        .one(&txn)
        .await?;
    if existing.is_some() {
        return Err(ProvisionError::DuplicateUsername(username));
    }

    let role = RoleActiveModel {
        tenant_id: Set(tenant.tenant_id),
        role_name: Set("超级管理员".to_string()),
        role_key: Set("admin".to_string()),
        role_sort: Set(1),
        data_scope: Set(1),
        status: Set(0),
        version: Set(0),
        remark: Set(Some("租户内置管理员".to_string())),
        create_by: Set(input.create_by.map(str::to_string)),
        create_time: Set(now),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    let user = UserActiveModel {
        tenant_id: Set(tenant.tenant_id),
        user_name: Set(username.clone()),
        nick_name: Set("管理员".to_string()),
        password: Set(password_hash),
        status: Set(0),
        version: Set(0),
        remark: Set(Some("租户初始管理员".to_string())),
        create_by: Set(input.create_by.map(str::to_string)),
        create_time: Set(now),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    UserIdentityActiveModel {
        tenant_id: Set(tenant.tenant_id),
        user_id: Set(user.user_id),
        identity_type: Set("Username".to_string()),
        identifier: Set(username),
        provider: Set(None),
        verified_at: Set(Some(now)),
        is_primary: Set(true),
        create_time: Set(now),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    UserRoleActiveModel {
        user_id: Set(user.user_id),
        role_id: Set(role.role_id),
    }
    .insert(&txn)
    .await?;

    // 菜单是全局共享的；租户管理员拿到本租户可用的菜单，但平台租户/套餐菜单属于
    // 默认租户的超级管理员，不能授予其它租户（否则会看到永远 403 的入口）。
    fn is_platform_menu(menu: &crate::entity::MenuModel) -> bool {
        if menu.menu_id == 5 {
            return true;
        }
        menu.perms.as_deref().is_some_and(|perm| {
            perm.starts_with("system:tenant:") || perm.starts_with("system:package:")
        })
    }
    for menu in MenuEntity::find().all(&txn).await? {
        if is_platform_menu(&menu) {
            continue;
        }
        RoleMenuActiveModel {
            role_id: Set(role.role_id),
            menu_id: Set(menu.menu_id),
        }
        .insert(&txn)
        .await?;
    }

    txn.commit().await?;
    Ok(ProvisionedTenant {
        tenant,
        admin_user_id: user.user_id,
    })
}
