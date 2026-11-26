//! 认证接口：登录、刷新、登出、当前用户、路由。

use axum::{Json, extract::State, http::StatusCode};
use chrono::Utc;
use rust_i18n::t;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use cicada_audit::LoginLog;
use cicada_auth::{AuthError, LockoutPolicy, SessionPolicy, lockout_key};
use cicada_kernel::{RequestContext, RoleId, TenantId};
use cicada_system::{
    TenantColumn, TenantEntity, UserActiveModel, UserColumn, UserEntity, UserModel,
    load_grant_snapshot, load_menu_tree,
};

use crate::extractor::RequestContextExt;
use crate::problem::{ApiErr, BizCode, FieldError, Problem};
use crate::state::AppState;

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    /// 登录标识符：账号名 / 手机 / 邮箱。
    pub username: String,
    pub password: String,
    /// 目标租户 id；缺省使用 `tenant.default_id`。多租户前端从 `/auth/tenants` 选择。
    #[serde(default)]
    pub tenant_id: Option<i64>,
    /// 启用 TOTP 的账号在此携带 6 位动态码。
    #[serde(default)]
    pub totp: Option<String>,
    /// `GET /auth/captcha` 发放的验证码 id（连续失败后必带）。
    #[serde(default)]
    pub captcha_id: Option<String>,
    /// 用户输入的验证码字符。
    #[serde(default)]
    pub captcha_code: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: i64,
    pub tenant_id: i64,
    pub username: String,
}

/// 刷新请求：只带刷新令牌（不透明，内含会话 id）。
#[derive(Deserialize, ToSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Serialize, ToSchema)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Serialize, ToSchema)]
pub struct ProfileResponse {
    pub user_id: i64,
    pub user_name: String,
    pub nick_name: String,
    pub email: Option<String>,
    pub phonenumber: Option<String>,
    pub tenant_id: i64,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateProfileRequest {
    pub nick_name: String,
    pub email: Option<String>,
    pub phonenumber: Option<String>,
}

/// 密码失败锁定策略（配置 `auth.password`）。
fn lockout_policy(state: &AppState) -> LockoutPolicy {
    LockoutPolicy {
        max_retry: state.config.auth.password.max_retry,
        lock_secs: state.config.auth.password.lock_minutes * 60,
    }
}

pub(crate) fn session_policy(state: &AppState) -> SessionPolicy {
    SessionPolicy {
        refresh_ttl_secs: state.config.auth.refresh_ttl_days * 24 * 60 * 60,
        max_sessions: state.config.auth.max_sessions,
    }
}

#[utoipa::path(
    post,
    path = "/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "登录成功", body = LoginResponse),
        (status = 422, description = "账号或密码错误", body = Problem),
        (status = 403, description = "账号已停用", body = Problem)
    )
)]
pub async fn login(
    State(state): State<AppState>,
    ctx: RequestContextExt,
    Json(req): Json<LoginRequest>,
) -> Result<(StatusCode, Json<LoginResponse>), ApiErr> {
    let ctx = ctx.into_inner();
    let ip = ctx.ip.to_string();
    let ua = ctx.user_agent.unwrap_or_default();
    let tenant = resolve_login_tenant(&state, req.tenant_id, ctx.tenant_id, &ctx.locale).await?;
    let tenant_id = tenant.get();

    // 图形验证码闸门：连续失败达到阈值后强制；前端主动带了也校验。
    let failures = state
        .lockout
        .failures(&lockout_key(tenant_id, &req.username))
        .await
        .unwrap_or(0);
    let provided = req.captcha_id.as_deref().is_some_and(|s| !s.is_empty())
        || req.captcha_code.as_deref().is_some_and(|s| !s.is_empty());
    if state.captcha.is_required(failures) || (state.captcha.params().enabled && provided) {
        let passed = state
            .captcha
            .verify(req.captcha_id.as_deref(), req.captcha_code.as_deref())
            .await
            .map_err(|e| map_auth_error(&ctx.locale, &e))?;
        if !passed {
            state
                .audit
                .record_login(LoginLog::failure(
                    tenant_id,
                    &req.username,
                    Some(ip.clone()).filter(|s| s != "0.0.0.0"),
                    Some(ua.clone()).filter(|s| !s.is_empty()),
                    "图形验证码错误或已过期",
                ))
                .await;
            return Err(Problem::new(
                BizCode::AuthCaptchaInvalid,
                t!("user.jcaptcha.error", locale = ctx.locale.as_str()).into_owned(),
            ));
        }
    }

    match cicada_auth::login(
        state.db.conn(),
        &state.tokens,
        state.sessions.as_ref(),
        state.lockout.as_ref(),
        session_policy(&state),
        lockout_policy(&state),
        tenant,
        &req.username,
        &req.password,
        req.totp.as_deref(),
        &ip,
        &ua,
    )
    .await
    {
        Ok(r) => {
            state
                .audit
                .record_login(LoginLog::success(
                    tenant_id,
                    &r.principal.user_name,
                    Some(ip.clone()).filter(|s| s != "0.0.0.0"),
                    Some(ua.clone()).filter(|s| !s.is_empty()),
                ))
                .await;
            Ok((
                StatusCode::OK,
                Json(LoginResponse {
                    access_token: r.access_token,
                    refresh_token: r.refresh_token,
                    user_id: r.principal.user_id.get(),
                    tenant_id: r.principal.tenant_id.get(),
                    username: r.principal.user_name,
                }),
            ))
        }
        Err(e) => {
            state
                .audit
                .record_login(LoginLog::failure(
                    tenant_id,
                    &req.username,
                    Some(ip.clone()).filter(|s| s != "0.0.0.0"),
                    Some(ua.clone()).filter(|s| !s.is_empty()),
                    e.to_string(),
                ))
                .await;
            Err(map_auth_error(&ctx.locale, &e))
        }
    }
}

/// 解析登录租户：显式 `tenant_id` 必须存在且启用；否则回落默认租户。
async fn resolve_login_tenant(
    state: &AppState,
    requested: Option<i64>,
    fallback: TenantId,
    locale: &str,
) -> Result<TenantId, ApiErr> {
    let Some(id) = requested else {
        return Ok(fallback);
    };
    let tenant = TenantEntity::find_by_id(id)
        .one(state.db.conn())
        .await
        .map_err(|e| tenant_db_error(locale, e))?;
    match tenant {
        Some(row) if row.status == 0 => Ok(TenantId::new(row.tenant_id)),
        _ => Err(Problem::new(
            BizCode::SysNotFound,
            t!("auth.tenant_invalid", locale = locale).into_owned(),
        )),
    }
}

fn tenant_db_error(locale: &str, e: impl std::fmt::Display) -> ApiErr {
    tracing::error!(error = %e, "租户查询失败");
    Problem::new(
        BizCode::Internal,
        t!("common.internal_error", locale = locale).into_owned(),
    )
}

#[derive(Serialize, ToSchema)]
pub struct LoginTenantVo {
    pub tenant_id: i64,
    pub tenant_name: String,
}

/// 登录页的租户选择数据：只暴露启用中的租户 id 与名称。
#[utoipa::path(
    get,
    path = "/auth/tenants",
    responses((status = 200, description = "启用中的租户", body = [LoginTenantVo]))
)]
pub async fn login_tenants(
    State(state): State<AppState>,
) -> Result<Json<Vec<LoginTenantVo>>, ApiErr> {
    let rows = TenantEntity::find()
        .filter(TenantColumn::Status.eq(0))
        .order_by_asc(TenantColumn::TenantId)
        .all(state.db.conn())
        .await
        .map_err(|e| tenant_db_error("zh-CN", e))?;
    Ok(Json(
        rows.into_iter()
            .map(|row| LoginTenantVo {
                tenant_id: row.tenant_id,
                tenant_name: row.tenant_name,
            })
            .collect(),
    ))
}

/// 图形验证码响应：`img` 为不带 data URI 前缀的 base64 PNG；
/// `enabled=false` 时 `captcha_id`/`img` 为空串（兼容既有客户端的 `captchaImage` 形态）。
#[derive(Serialize, ToSchema)]
pub struct CaptchaVo {
    pub captcha_id: String,
    pub img: String,
    pub enabled: bool,
}

#[utoipa::path(
    get,
    path = "/auth/captcha",
    responses((status = 200, description = "图形验证码", body = CaptchaVo))
)]
pub async fn captcha_image(
    State(state): State<AppState>,
    ctx: RequestContextExt,
) -> Result<(StatusCode, Json<CaptchaVo>), ApiErr> {
    let ctx = ctx.into_inner();
    if !state.captcha.params().enabled {
        return Ok((
            StatusCode::OK,
            Json(CaptchaVo {
                captcha_id: String::new(),
                img: String::new(),
                enabled: false,
            }),
        ));
    }
    let (captcha_id, img) = state
        .captcha
        .generate()
        .await
        .map_err(|e| map_auth_error(&ctx.locale, &e))?;
    Ok((
        StatusCode::OK,
        Json(CaptchaVo {
            captcha_id,
            img,
            enabled: true,
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/auth/refresh",
    request_body = RefreshRequest,
    responses(
        (status = 200, description = "刷新成功", body = RefreshResponse),
        (status = 401, description = "刷新令牌无效或已被轮换", body = Problem)
    )
)]
pub async fn refresh(
    State(state): State<AppState>,
    ctx: RequestContextExt,
    Json(req): Json<RefreshRequest>,
) -> Result<(StatusCode, Json<RefreshResponse>), ApiErr> {
    let ctx = ctx.into_inner();
    let ip = ctx.ip.to_string();
    let ua = ctx.user_agent.unwrap_or_default();
    match cicada_auth::refresh(
        &state.tokens,
        state.sessions.as_ref(),
        session_policy(&state),
        &req.refresh_token,
        &ip,
        &ua,
    )
    .await
    {
        Ok(r) => Ok((
            StatusCode::OK,
            Json(RefreshResponse {
                access_token: r.access_token,
                refresh_token: r.refresh_token,
            }),
        )),
        Err(e) => Err(map_auth_error(&ctx.locale, &e)),
    }
}

#[utoipa::path(
    post,
    path = "/auth/logout",
    responses(
        (status = 204, description = "已登出"),
        (status = 401, description = "未认证", body = Problem)
    )
)]
pub async fn logout(
    State(state): State<AppState>,
    ctx: RequestContextExt,
) -> Result<StatusCode, ApiErr> {
    let ctx = ctx.into_inner();
    let Some(principal) = &ctx.principal else {
        return Err(unauthorized(&ctx.locale));
    };
    match cicada_auth::logout(state.sessions.as_ref(), principal.session_id.as_str()).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err(map_auth_error(&ctx.locale, &e)),
    }
}

/// `AuthError` → 业务码目录业务码 + 已本地化 `detail`。
///
/// 状态码由业务码给出，调用点只需 `Err(map_auth_error(..))`。相对旧的简化错误体，
/// 两处状态按业务码目录归位：TOTP 校验失败 401→422（`CICADA_AUTH_1006`）、身份重复绑定
/// 422→409（`CICADA_AUTH_1008`）。
pub(crate) fn map_auth_error(locale: &str, e: &AuthError) -> ApiErr {
    // 面向用户的消息走 rust-i18n；技术错误（Db/Session/Password）细节只进日志，
    // 对外给 `common.internal_error`——500 级不泄露内部信息。
    let (code, detail) = match e {
        AuthError::InvalidCredentials => (
            BizCode::AuthBadCredentials,
            t!("user.password.not.match", locale = locale).into_owned(),
        ),
        AuthError::Disabled => (
            BizCode::AuthDisabled,
            t!("user.blocked", locale = locale).into_owned(),
        ),
        AuthError::AccountLocked { retry_after_secs } => (
            BizCode::AuthLocked,
            t!(
                "user.password.retry.limit.exceed",
                locale = locale,
                seconds = *retry_after_secs
            )
            .into_owned(),
        ),
        AuthError::InvalidToken => (
            BizCode::AuthUnauthorized,
            t!("auth.invalid_token", locale = locale).into_owned(),
        ),
        AuthError::SessionExpired => (
            BizCode::AuthUnauthorized,
            t!("auth.session_expired", locale = locale).into_owned(),
        ),
        AuthError::RefreshReplay => (
            BizCode::AuthRefreshInvalid,
            t!("auth.refresh_replay", locale = locale).into_owned(),
        ),
        AuthError::TotpRequired => (
            BizCode::AuthUnauthorized,
            t!("auth.totp.required", locale = locale).into_owned(),
        ),
        AuthError::TotpInvalid => (
            BizCode::AuthTotpFailed,
            t!("auth.totp.invalid", locale = locale).into_owned(),
        ),
        AuthError::IdentityNotFound => (
            BizCode::SysNotFound,
            t!("auth.identity.not_found", locale = locale).into_owned(),
        ),
        AuthError::LastIdentity => (
            BizCode::AuthLastIdentity,
            t!("auth.identity.last", locale = locale).into_owned(),
        ),
        // 未启用 / 已启用属于状态冲突：业务码目录没有专用码，挂 1006 并保留 409 语义。
        AuthError::TotpNotEnabled => (
            BizCode::AuthTotpFailed,
            t!("auth.totp.not_enabled", locale = locale).into_owned(),
        ),
        AuthError::TotpAlreadyEnabled => (
            BizCode::AuthTotpFailed,
            t!("auth.totp.already_enabled", locale = locale).into_owned(),
        ),
        AuthError::DuplicateIdentity => (
            BizCode::AuthIdentityBound,
            t!("auth.identity.duplicate", locale = locale).into_owned(),
        ),
        AuthError::UnsupportedIdentityType => (
            BizCode::ValidationFields,
            t!("auth.identity.unsupported", locale = locale).into_owned(),
        ),
        AuthError::InvalidIdentifier => (
            BizCode::ValidationFields,
            t!("auth.identity.invalid", locale = locale).into_owned(),
        ),
        AuthError::VerificationFailed => (
            BizCode::ValidationFields,
            t!("auth.verification_failed", locale = locale).into_owned(),
        ),
        AuthError::Jwt(_) => (
            BizCode::AuthUnauthorized,
            t!("auth.invalid_token", locale = locale).into_owned(),
        ),
        AuthError::OAuthProviderNotFound(provider) => (
            BizCode::SysNotFound,
            t!(
                "auth.oauth.provider_not_found",
                locale = locale,
                provider = provider
            )
            .into_owned(),
        ),
        AuthError::OAuthNativeUnsupported(provider) => (
            BizCode::ValidationFields,
            t!(
                "auth.oauth.native_unsupported",
                locale = locale,
                provider = provider
            )
            .into_owned(),
        ),
        // state 失效可能是 CSRF 或过期，统一按未认证处理（401 语义）。
        AuthError::OAuthStateInvalid => (
            BizCode::AuthUnauthorized,
            t!("auth.oauth.state_invalid", locale = locale).into_owned(),
        ),
        AuthError::OAuthUpstream(detail) => (
            BizCode::AuthUpstreamUnavailable,
            t!("auth.oauth.upstream", locale = locale, detail = detail).into_owned(),
        ),
        other => {
            tracing::error!(error = %other, "认证内部错误");
            (
                BizCode::Internal,
                t!("common.internal_error", locale = locale).into_owned(),
            )
        }
    };
    let problem = Problem::new(code, detail);
    if matches!(e, AuthError::TotpNotEnabled | AuthError::TotpAlreadyEnabled) {
        problem.with_status(StatusCode::CONFLICT)
    } else {
        problem
    }
}

#[utoipa::path(
    get,
    path = "/auth/profile",
    responses(
        (status = 200, description = "当前用户信息", body = ProfileResponse),
        (status = 401, description = "未认证", body = Problem)
    )
)]
pub async fn profile(
    State(state): State<AppState>,
    ctx: RequestContextExt,
) -> Result<Json<ProfileResponse>, ApiErr> {
    let ctx = ctx.into_inner();
    let Some(principal) = &ctx.principal else {
        return Err(unauthorized(&ctx.locale));
    };
    let user = current_user(&state, &ctx).await?;
    Ok(Json(ProfileResponse {
        user_id: principal.user_id.get(),
        user_name: principal.user_name.clone(),
        nick_name: user.nick_name,
        email: user.email,
        phonenumber: user.phonenumber,
        tenant_id: principal.tenant_id.get(),
        roles: ctx.roles.clone(),
        permissions: ctx
            .permissions
            .iter()
            .map(|p| p.as_str().to_string())
            .collect(),
    }))
}

/// 当前登录用户在本租户内的实体行。
async fn current_user(state: &AppState, ctx: &RequestContext) -> Result<UserModel, ApiErr> {
    let Some(principal) = &ctx.principal else {
        return Err(unauthorized(&ctx.locale));
    };
    UserEntity::find()
        .filter(UserColumn::UserId.eq(principal.user_id.get()))
        .filter(UserColumn::TenantId.eq(ctx.tenant_id.get()))
        .filter(UserColumn::DeletedAt.is_null())
        .one(state.db.conn())
        .await
        .map_err(|e| user_db_error(&ctx.locale, e))?
        .ok_or_else(|| {
            Problem::new(
                BizCode::SysNotFound,
                t!("common.not_found", locale = ctx.locale.as_str()).into_owned(),
            )
        })
}

fn user_db_error(locale: &str, e: impl std::fmt::Display) -> ApiErr {
    tracing::error!(error = %e, "用户资料数据库操作失败");
    Problem::new(
        BizCode::Internal,
        t!("common.internal_error", locale = locale).into_owned(),
    )
}

fn field_problem(field: &str, message: String) -> ApiErr {
    Problem::new(BizCode::ValidationFields, message.clone())
        .fields(vec![FieldError::new(field, message)])
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[utoipa::path(
    put,
    path = "/auth/password",
    request_body = ChangePasswordRequest,
    responses(
        (status = 204, description = "密码已更新，所有会话已撤销"),
        (status = 401, description = "未认证", body = Problem),
        (status = 422, description = "当前密码错误或新密码不合规", body = Problem)
    )
)]
#[cicada_macros::audit(module = "auth", action = "password:change", kind = "update")]
pub async fn change_password(
    State(state): State<AppState>,
    ctx: RequestContextExt,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<StatusCode, ApiErr> {
    let ctx = ctx.into_inner();
    let Some(principal) = &ctx.principal else {
        return Err(unauthorized(&ctx.locale));
    };
    let user = current_user(&state, &ctx).await?;
    let matches = cicada_auth::verify_password(&req.old_password, &user.password)
        .map_err(|e| map_auth_error(&ctx.locale, &e))?;
    if !matches {
        return Err(field_problem(
            "old_password",
            t!("auth.password.old_wrong", locale = ctx.locale.as_str()).into_owned(),
        ));
    }
    if req.new_password == req.old_password {
        return Err(field_problem(
            "new_password",
            t!("auth.password.same", locale = ctx.locale.as_str()).into_owned(),
        ));
    }
    let min_length = state.config.auth.password.min_length;
    let problems = cicada_infra::password_problems(&req.new_password, min_length);
    if !problems.is_empty() {
        return Err(field_problem(
            "new_password",
            cicada_infra::password_problem_text(&problems, min_length),
        ));
    }
    let hash = cicada_auth::hash_password(&req.new_password)
        .map_err(|e| map_auth_error(&ctx.locale, &e))?;
    UserActiveModel {
        user_id: Set(user.user_id),
        tenant_id: Set(user.tenant_id),
        password: Set(hash),
        update_time: Set(Some(Utc::now())),
        ..Default::default()
    }
    .update(state.db.conn())
    .await
    .map_err(|e| user_db_error(&ctx.locale, e))?;
    // 改密即失效：所有会话（含当前）都不再可信。
    if let Err(e) = state.sessions.revoke_user(principal.user_id.get()).await {
        tracing::warn!(error = %e, "改密后撤销会话失败");
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/auth/profile",
    request_body = UpdateProfileRequest,
    responses(
        (status = 204, description = "资料已更新"),
        (status = 401, description = "未认证", body = Problem),
        (status = 422, description = "昵称或邮箱格式不合规", body = Problem)
    )
)]
#[cicada_macros::audit(module = "auth", action = "profile:update", kind = "update")]
pub async fn update_profile(
    State(state): State<AppState>,
    ctx: RequestContextExt,
    Json(req): Json<UpdateProfileRequest>,
) -> Result<StatusCode, ApiErr> {
    let ctx = ctx.into_inner();
    let user = current_user(&state, &ctx).await?;
    let nick_name = req.nick_name.trim();
    if nick_name.is_empty() {
        return Err(field_problem("nick_name", "昵称不能为空".to_string()));
    }
    let email = clean_optional(req.email);
    if email.as_deref().is_some_and(|value| !value.contains('@')) {
        return Err(field_problem("email", "邮箱格式不正确".to_string()));
    }
    UserActiveModel {
        user_id: Set(user.user_id),
        tenant_id: Set(user.tenant_id),
        nick_name: Set(nick_name.to_string()),
        email: Set(email),
        phonenumber: Set(clean_optional(req.phonenumber)),
        update_time: Set(Some(Utc::now())),
        ..Default::default()
    }
    .update(state.db.conn())
    .await
    .map_err(|e| user_db_error(&ctx.locale, e))?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/auth/routers",
    responses(
        (status = 200, description = "当前用户可见的菜单树"),
        (status = 401, description = "未认证", body = Problem)
    )
)]
pub async fn routers(
    State(state): State<AppState>,
    ctx: RequestContextExt,
) -> Result<Json<Vec<cicada_system::MenuTreeNode>>, ApiErr> {
    let ctx = ctx.into_inner();
    let Some(principal) = &ctx.principal else {
        return Err(unauthorized(&ctx.locale));
    };
    let snapshot =
        load_grant_snapshot(state.db.conn(), principal.tenant_id, principal.user_id).await;
    let role_ids: Vec<RoleId> = snapshot.roles.iter().map(|r| r.role_id).collect();
    // 菜单树按「角色集合」缓存（缓存对象之一）：一次登录一份，翻页与刷新都复用。
    // 键里带着域版本号，所以菜单或角色变更时整域的版本一跳就够，不需要删键。
    let ident = cicada_system::roles_ident(&role_ids.iter().map(|r| r.get()).collect::<Vec<_>>());
    let tree = cicada_system::cached_json(
        &state.sys_cache,
        cicada_system::CacheDomain::Menu,
        None,
        &ident,
        || async { Some(load_menu_tree(state.db.conn(), &role_ids).await) },
    )
    .await
    .unwrap_or_default();
    Ok(Json(tree))
}

fn unauthorized(locale: &str) -> ApiErr {
    Problem::new(
        BizCode::AuthUnauthorized,
        t!("auth.unauthorized", locale = locale).into_owned(),
    )
}
