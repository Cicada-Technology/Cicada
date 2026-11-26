# cicada-notify

通知中心。L2 领域模块，拥有 `sys_notify_*` 三张表。

## 职责

- 渠道（`sys_notify_channel`）：`code` + `kind`（`sms` / `email` / `webhook` / `im`）+ `config`。
- 模板（`sys_notify_template`）：`code` + 渠道类型 + 语言 + minijinja 标题/正文。
- 记录（`sys_notify_record`）：每次投递留痕，失败也留痕（`status` 1 已发 / 2 失败 + `error`）。
- `NotifyService::send`：选渠道 → 选模板（精确语言 → `zh-CN` → 第一个）→ 渲染 → 投递 → 回写记录。

## 公开 API

- `NotifyService`：`new(db, Arc<dyn NotifySender>)`、`send(tenant, SendRequest)`、`choose_locale`。
- `NotifySender`（缝）：`RoutingSender`（email 配 `host` 走 SMTP，否则走 HTTP 网关）、
  `HttpSender`、`SmtpSender`、`RecordingSender`（测试）。
- `ChannelKind` / `ChannelConfig` / `encode_body`：sms / email / webhook / im 的传输约定。
- `render` / `validate`：minijinja 渲染与「保存时语法校验」（严格模式，未知变量即报错）。
- `entity::*`：三张表的 SeaORM 实体。

## 依赖

`cicada-kernel` / `cicada-infra` / `cicada-macros` / `sea-orm` / `minijinja` / `reqwest`。

## 不做什么

- 不自己做重试与限流：失败只留痕 + `warn` 日志，重试策略交给调用方或调度器。
- 不做厂商 SDK：sms / webhook / im 统一走 HTTP 网关，换厂商只需换 `config.endpoint`；
  email 已内置 lettre SMTP（配 `host`）与 HTTP 网关两种后端。
- 不拥有 `sys_oper_log`：审计走 `cicada-audit`。
