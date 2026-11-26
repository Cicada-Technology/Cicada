use argon2::{Argon2, PasswordHasher};
use sea_orm_migration::sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, Statement,
};
use sea_orm_migration::sea_query::{Alias, Expr, OnConflict, Query, Value};

/// 初始管理员账号（init-admin 档）。
pub const ADMIN_USERNAME: &str = "admin";
pub const ADMIN_PASSWORD: &str = "Cicada@2026";
pub const DEFAULT_TENANT_ID: i64 = 1;

fn v<T: Into<Value>>(x: T) -> Expr {
    Expr::Value(x.into())
}

async fn insert_row(
    db: &DatabaseConnection,
    table: &str,
    _pk: &str,
    columns: &[&str],
    values: Vec<Expr>,
) -> Result<(), DbErr> {
    let cols: Vec<Alias> = columns.iter().map(|c| Alias::new(*c)).collect();
    let insert = Query::insert()
        .into_table(Alias::new(table))
        .columns(cols)
        .values_panic(values)
        .on_conflict(OnConflict::new().do_nothing().to_owned())
        .to_owned();
    db.execute(&insert).await?;
    Ok(())
}

fn hash_password(pw: &str) -> Result<String, argon2::password_hash::Error> {
    Ok(Argon2::default().hash_password(pw.as_bytes())?.to_string())
}

/// 幂等：创建默认租户、超级管理员角色、admin 账号、身份绑定、角色-用户、角色-全菜单。
/// 口令规则同样管住 CLI：管理员账号是弱口令最贵的地方，
/// 「先建起来再改」在实际操作里几乎不会发生。
fn assert_password_policy(pw: &str) -> Result<(), DbErr> {
    use cicada_infra::config::PasswordConfig;
    let min_length = PasswordConfig::default().min_length;
    let problems = cicada_infra::password_problems(pw, min_length);
    if problems.is_empty() {
        return Ok(());
    }
    Err(DbErr::Custom(cicada_infra::password_problem_text(
        &problems, min_length,
    )))
}

pub async fn init_admin(db: &DatabaseConnection) -> Result<(), DbErr> {
    // 默认租户
    insert_row(
        db,
        "sys_tenant",
        "tenant_id",
        &[
            "tenant_id",
            "tenant_name",
            "status",
            "remark",
            "create_by",
            "create_time",
            "update_by",
            "update_time",
        ],
        vec![
            v(DEFAULT_TENANT_ID),
            v("默认租户"),
            v(0i16),
            v("系统默认租户"),
            v("admin"),
            v(chrono::Utc::now()),
            v(Value::String(None)),
            v(Value::ChronoDateTimeUtc(None)),
        ],
    )
    .await?;

    // 超级管理员角色（data_scope = 1 全部）
    insert_row(
        db,
        "sys_role",
        "role_id",
        &[
            "role_id",
            "tenant_id",
            "role_name",
            "role_key",
            "role_sort",
            "data_scope",
            "status",
            "version",
            "deleted_at",
            "remark",
            "create_by",
            "create_time",
            "update_by",
            "update_time",
        ],
        vec![
            v(1i64),
            v(DEFAULT_TENANT_ID),
            v("超级管理员"),
            v("admin"),
            v(1i32),
            v(1i16),
            v(0i16),
            v(0i32),
            v(Value::ChronoDateTimeUtc(None)),
            v("内置超级管理员"),
            v("admin"),
            v(chrono::Utc::now()),
            v(Value::String(None)),
            v(Value::ChronoDateTimeUtc(None)),
        ],
    )
    .await?;

    // admin 账号
    assert_password_policy(ADMIN_PASSWORD)?;
    let pw_hash = hash_password(ADMIN_PASSWORD).map_err(|e| DbErr::Custom(e.to_string()))?;
    insert_row(
        db,
        "sys_user",
        "user_id",
        &[
            "user_id",
            "tenant_id",
            "user_name",
            "nick_name",
            "dept_id",
            "password",
            "email",
            "phonenumber",
            "status",
            "login_ip",
            "login_date",
            "version",
            "deleted_at",
            "remark",
            "create_by",
            "create_time",
            "update_by",
            "update_time",
        ],
        vec![
            v(1i64),
            v(DEFAULT_TENANT_ID),
            v(ADMIN_USERNAME),
            v("管理员"),
            v(Value::BigInt(None)),
            v(pw_hash),
            v(Value::String(None)),
            v(Value::String(None)),
            v(0i16),
            v(Value::String(None)),
            v(Value::ChronoDateTimeUtc(None)),
            v(0i32),
            v(Value::ChronoDateTimeUtc(None)),
            v("初始管理员"),
            v("admin"),
            v(chrono::Utc::now()),
            v(Value::String(None)),
            v(Value::ChronoDateTimeUtc(None)),
        ],
    )
    .await?;

    // 用户名身份绑定
    insert_row(
        db,
        "sys_user_identity",
        "identity_id",
        &[
            "identity_id",
            "tenant_id",
            "user_id",
            "identity_type",
            "identifier",
            "provider",
            "verified_at",
            "is_primary",
            "create_time",
        ],
        vec![
            v(1i64),
            v(DEFAULT_TENANT_ID),
            v(1i64),
            v("Username"),
            v(ADMIN_USERNAME),
            v(Value::String(None)),
            v(chrono::Utc::now()),
            v(true),
            v(chrono::Utc::now()),
        ],
    )
    .await?;

    // 用户-角色
    insert_row(
        db,
        "sys_user_role",
        "user_id",
        &["user_id", "role_id"],
        vec![v(1i64), v(1i64)],
    )
    .await?;

    // 角色-全菜单：以库里的实际菜单为准（含目录、菜单与按钮）。
    for menu_id in all_menu_ids(db).await? {
        insert_row(
            db,
            "sys_role_menu",
            "role_id",
            &["role_id", "menu_id"],
            vec![v(1i64), v(menu_id)],
        )
        .await?;
    }

    // 上面显式写了主键，重置序列以免后续自增插入冲突。
    crate::seed::reset_sequences(db).await?;
    Ok(())
}

/// 全局菜单集合（`sys_menu` 不带 `tenant_id`，各租户共享同一套菜单结构）。
/// 当前库里的全部菜单 id（含目录、菜单与按钮）。
///
/// 不写死范围：菜单会随迁移增长，硬编码的区间让新模块在管理员侧栏里静默消失。
async fn all_menu_ids(db: &DatabaseConnection) -> Result<Vec<i64>, DbErr> {
    let rows = db
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT menu_id FROM sys_menu ORDER BY menu_id",
        ))
        .await?;
    rows.into_iter()
        .map(|row| row.try_get_by_index::<i64>(0))
        .collect()
}

/// 重置某个账号的密码（`admin reset`）。返回是否有行被更新。
pub async fn reset_password(
    db: &DatabaseConnection,
    tenant_id: i64,
    user_name: &str,
    new_password: &str,
) -> Result<bool, DbErr> {
    assert_password_policy(new_password)?;
    let pw_hash = hash_password(new_password).map_err(|e| DbErr::Custom(e.to_string()))?;
    let res = db
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE sys_user SET password = $1, update_time = now()              WHERE tenant_id = $2 AND user_name = $3 AND deleted_at IS NULL",
            [
                pw_hash.into(),
                tenant_id.into(),
                user_name.into(),
            ],
        ))
        .await?;
    Ok(res.rows_affected() > 0)
}
