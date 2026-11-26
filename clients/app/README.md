# cicada Tauri 客户端（桌面 + Android + iOS）

Tauri 2 + Rust + TypeScript。**一套前端、一套 REST、四个运行面**：Web 由 Nginx 静态托管，
桌面（Linux/macOS/Windows）、Android、iOS 都由本目录的同一个 Tauri 工程加载 `web/` 的
同一份 Vue 构建产物。壳层只补浏览器做不到的三件事：

- 会话令牌写入平台安全存储（`secure_load` / `secure_save` / `secure_clear`）：
  Linux Secret Service、Windows Credential Manager、macOS/iOS Keychain、Android Keystore；
- 第三方登录交给系统浏览器，`cicada://oauth/callback` 由 deep link 接回应用；
- 文件下载落盘（`save_file`）与 `app_info` 诊断。

## 目录

```text
clients/app/
├── package.json                  # @tauri-apps/cli + 各平台脚本
└── src-tauri/
    ├── Cargo.toml                # tauri 2 + deep-link + opener + keyring-core 平台 store
    ├── tauri.conf.json           # 窗口、CSP、deep-link scheme、dev/build 命令
    ├── tauri.android.conf.json   # Android 的 beforeBuildCommand（build:android）
    ├── tauri.ios.conf.json       # iOS 的 beforeBuildCommand（build:ios）
    ├── capabilities/             # Tauri 2 权限
    ├── gen/android/              # Android Studio 工程（入库）
    ├── gen/apple/                # Xcode 工程（macOS 上 tauri ios init 生成，入库）
    └── src/
        ├── main.rs
        └── lib.rs                # keyring / 打开外链 / 下载 / deep link
```

## 先起后端

```bash
# 仓库根目录
docker compose up -d postgres redis
cargo run -p cicada-cli -- migrate
cargo run -p cicada-cli -- seed
cargo run -p cicada-cli -- init-admin
cargo run -p cicada-server
```

## 桌面（Linux/macOS/Windows）

```bash
cd clients/app
corepack pnpm install
corepack pnpm dev      # 自动起 web/ 的 Vite dev server（5173）
corepack pnpm build    # deb/rpm/AppImage 或 .msi/.dmg/.app
```

桌面构建执行 `web/` 的 `build:desktop`，读取 `web/.env.desktop`，默认 API
`http://127.0.0.1:8081`；连接远程后端时覆盖：

```bash
VITE_API_BASE_URL=https://api.example.com corepack pnpm build
```

## Android

前置：Android Studio / SDK Platform（Tauri 当前模板使用 compileSdk 37）、
Build-Tools、NDK（side by side）、JDK 17+，并设置：

```bash
export ANDROID_HOME="$HOME/Android/Sdk"
export NDK_HOME="$ANDROID_HOME/ndk/$(ls -1 "$ANDROID_HOME/ndk" | tail -1)"
```

首次初始化（新工程或升级 Tauri 模板后运行；本框架仓库已生成 `gen/android`，克隆后通常可直接构建）：

```bash
cd clients/app
corepack pnpm android:init       # 生成/更新 gen/android
corepack pnpm android:dev        # 模拟器/真机开发
corepack pnpm android:build -- --apk   # 生成 APK（release 需签名配置）
```

Android 构建执行 `web/` 的 `build:android`：模拟器默认 API
`http://10.0.2.2:8081`（宿主机别名）。真机必须覆盖成局域网/公网地址：

```bash
VITE_API_BASE_URL=https://api.example.com corepack pnpm android:build -- --apk
```

Debug 包允许明文 HTTP（便于连本机后端）；release 包默认禁止明文，生产 API 必须是 HTTPS。
真机用局域网明文调试时，需要把该地址加入 `tauri.conf.json` 的 CSP `connect-src`。

## iOS

前置：macOS + 完整 Xcode + CocoaPods + Rust iOS targets：

```bash
rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim
```

首次初始化与运行（Linux 上没有 iOS 构建链，`tauri ios` 只在 macOS 可用）：

```bash
cd clients/app
corepack pnpm ios:init
corepack pnpm ios:dev
corepack pnpm ios:build
```

iOS 构建执行 `web/` 的 `build:ios`：模拟器默认 API `http://127.0.0.1:8081`。
真机覆盖 `VITE_API_BASE_URL=https://api.example.com`。未签名的设备包需要在 Xcode 里
配置 Team / Bundle Identifier；`gen/apple` 生成后应随工程入库。
若调试期必须走明文 HTTP，需在 `gen/apple` 的 Info.plist 增加 ATS
`NSAllowsLocalNetworking`；生产环境仍应使用 HTTPS。

## Web

`web/` 独立构建、由 Nginx/CDN 托管；Tauri 三端复用同一份产物。
响应式断点在 `web/src/layout/AppLayout.vue`（≤900px 抽屉侧栏）与
`web/src/styles/base.css`（≤680px 弹层/表单/安全区）。

```bash
cd web
corepack pnpm install
corepack pnpm dev       # http://localhost:5173
corepack pnpm build     # 静态产物 dist/
```

## 第三方登录（deep link）

原生端不在 webview 里打开 IdP 页面，而是：

1. 调 `GET /auth/login/{provider}/authorize-url?client=native` 取授权地址；
2. 用系统浏览器打开（`open_external`，`tauri-plugin-opener`）；
3. IdP 回跳 `cicada://oauth/callback?code=…&state=…`：
   - 桌面：deep-link 插件接收；Linux/Windows 第二次启动由 `single-instance` 转发给主实例；
   - Android/iOS：系统把 URL 交给应用，deep-link 插件接收；
   - 冷启动时 URL 先缓存在 Rust 侧，由前端挂载时取走；
4. 前端进入 `/oauth/callback`，带 `client: "native"` 调回调接口换令牌。

后端配置里每个提供方同时登记浏览器与原生回调：

```yaml
oauth:
  providers:
    - code: keycloak
      # ...
      redirect_uri: "https://admin.example.com/oauth/callback"
      native_redirect_uri: "cicada://oauth/callback"
```

- deb/rpm 安装时注册 scheme；AppImage 首次启动时 `register_all` 自注册（需要 `xdg-mime`）。
- Android/iOS 的 scheme 由 `tauri.conf.json` 的 `plugins.deep-link.mobile` 在构建时写入
  AndroidManifest / Info.plist。
- 未配置 `native_redirect_uri` 时授权接口返回 422，不会静默回落到浏览器回调。

## 令牌存储

- 浏览器：localStorage（`web/src/utils/session.ts`）。
- 原生端：平台安全存储，服务名 `dev.cicada.client`，账号 `session`。
- 适配层在 `web/src/utils/native.ts`；页面与 API 层不感知运行环境。

Linux 需要可用的 Secret Service（GNOME Keyring / KWallet）；不可用时退回
localStorage 语义（见 `loadNativeTokens` 的兜底），不影响登录。
