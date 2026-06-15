# Atlas 产品身份完整迁移设计

## 目标

将当前桌面应用完整迁移为 Atlas 产品身份。Atlas 不是旧品牌的兼容升级版，而是新的产品身份、运行身份和协议身份。

完成后，仓库内不应再存在面向产品、代码、运行时、协议、资产、打包发布的旧品牌命名。Atlas 只认新的目录、数据库、环境变量、协议名和注入文件。

## 已确认决策

- 旧数据彻底断开：Atlas 不读取、不迁移、不 fallback 到旧品牌 home 目录或旧品牌数据库文件。
- 旧环境变量彻底断开：Atlas 不读取旧品牌环境变量前缀。
- 旧协议彻底断开：Atlas 不注册或生成旧品牌 MCP/server/sentinel/hook 名。
- 旧资产彻底断开：应用入口、文档、图标、截图引用不再指向旧品牌资产。
- 当前分支继续处理：保留已完成的大范围迁移改动，不从干净 base 重做。

## 非目标

- 不做旧用户数据迁移。
- 不保留旧配置兼容层。
- 不保留旧 MCP 协议别名。
- 不把这次迁移扩大成新的产品功能设计。
- 不清理与 Atlas 命名无关的历史表结构或 coding legacy 模型。

## 迁移范围

### 产品展示层

用户可见的产品身份必须统一为 Atlas：

- HTML title、Tauri productName、窗口标题。
- README、中文 README、架构文档、产品文档、设计文档。
- i18n 中英文文案。
- 品牌图标、public assets、README 截图引用。
- CSS class、DOM id、localStorage key、浏览器事件名中的产品前缀。

验收标准：

- 文本扫描不再出现旧品牌名的 PascalCase、lowercase、UPPERCASE 变体。
- 文件名扫描不再出现旧品牌名资产名。
- Vite 入口 title 为 `Atlas`，favicon 指向 Atlas asset。

### 代码身份层

代码内部产品身份必须统一为 Atlas：

- `package.json` package name。
- Rust crate name、library name、test import。
- helper、guard、constant、test function 中的产品前缀。
- comments 和 error/context message 中的产品身份。
- generated lockfile 中对应 package name。

验收标准：

- Rust 编译和测试通过。
- TypeScript build 通过。
- 旧产品名扫描无残留。

### 运行身份层

运行时只认 Atlas：

- 默认 home 目录为 `~/.atlas`。
- 默认数据库为 `atlas.db`。
- 环境变量只使用 `ATLAS_*`。
- Tauri identifier 使用 Atlas reverse domain。
- Keychain / power management / app id 使用 Atlas 产品身份。

验收标准：

- 路径相关单元测试覆盖 `atlas_home` 和 `atlas.db`。
- 环境变量扫描不再出现旧品牌环境变量前缀。
- 代码中不存在读取旧 home、旧 db 或旧 env 的 fallback 分支。

### Agent 与协议层

Agent 运行、MCP 注入和 ask/planner/global 协议必须统一为 Atlas：

- MCP server name 和 config 文件使用 `atlas_*` / `atlas-*`。
- planner/global skill layer 使用 Atlas 命名。
- sentinel 标签使用 `<atlas:...>`。
- ask hook、settings、opencode plugin 文件使用 Atlas 文件名。
- prompt 和注入说明中不再要求或引用旧品牌协议名。

验收标准：

- sentinel parser 测试覆盖 Atlas 标签。
- bus/inject 相关测试通过。
- 文件名扫描不再出现旧品牌 dotfile、bus、planner、global、ask hook 前缀。

### 打包与发布层

打包后的应用身份必须统一为 Atlas：

- `tauri.conf.json` 使用 Atlas productName 和 identifier。
- updater endpoint 指向 Atlas release path。
- app icons 使用 Atlas 图标。
- public assets 中删除旧品牌 SVG，使用 Atlas PNG/SVG。

验收标准：

- `pnpm build` 通过。
- `cargo test --manifest-path src-tauri/Cargo.toml --no-run` 通过。
- Atlas public assets HTTP 入口返回 200。

### 文档与资产层

文档必须描述 Atlas 当前产品身份，不应把旧品牌作为当前产品名。

允许保留的旧名只有一种情况：如果未来专门写迁移历史或 release note，可以把旧品牌当历史名提及。但本次迁移目标文件中不保留这种历史说明，避免误导使用者。

验收标准：

- 文档和 SVG 源文本旧名扫描无残留。
- README 图片引用存在且可访问。

## 实施策略

采用当前 diff 上的系统审计和补齐，不重做。

1. 建立旧名扫描清单。
   - 文本：旧品牌名的 PascalCase、lowercase、UPPERCASE 变体，以及旧 reverse domain、旧数据库名、旧 home 目录、旧 dash/underscore/colon 前缀。
   - 文件名：旧品牌名的 PascalCase、lowercase、UPPERCASE 变体。
   - 排除目录：`.git`、`node_modules`、`src-tauri/target`、`build`。

2. 按层级审计当前改动。
   - 产品展示层。
   - 代码身份层。
   - 运行身份层。
   - Agent 与协议层。
   - 打包与发布层。
   - 文档与资产层。

3. 对漏项做定向修复。
   - 只修 Atlas 迁移相关问题。
   - 不引入旧名兼容层。
   - 不做无关重构。

4. 执行完整验证。
   - 旧名文本扫描。
   - 旧名文件名扫描。
   - `pnpm build`。
   - `cargo test --manifest-path src-tauri/Cargo.toml`。
   - `cargo test --manifest-path src-tauri/Cargo.toml --no-run`。
   - `git diff --check`。
   - Vite 入口 title/favicon/static asset 检查。

## 错误处理

- 如果发现旧名只存在于构建输出或 target 目录，清理或排除生成物，不作为源码残留。
- 如果发现旧名出现在第三方依赖，排除依赖目录，不修改 vendored/generated 依赖。
- 如果发现旧名出现在数据库 migration 历史表名，先判断是否产品身份暴露。仅当它影响 fresh DB 的 Atlas 体验或测试时才改；否则不为了命名改历史 schema。
- 如果发现普通浏览器中的 Tauri `invoke/listen` 报错，只记录为浏览器环境限制。真正运行验收以 build/test/Tauri 编译路径为准。

## 验收矩阵

| 层级 | 检查项 | 通过条件 |
| --- | --- | --- |
| 产品展示 | title、README、i18n、CSS/event/localStorage | 旧产品名无残留，入口显示 Atlas |
| 代码身份 | package、crate、lib、helper、tests | build/test 通过，旧命名无残留 |
| 运行身份 | home、db、env、identifier | 只使用 `~/.atlas`、`atlas.db`、`ATLAS_*`、Atlas identifier |
| Agent 协议 | MCP、sentinel、skill layer、ask hook、opencode plugin | 只生成 Atlas 协议和文件名，测试通过 |
| 打包发布 | Tauri config、icons、updater、public assets | Atlas assets 存在，旧 assets 删除 |
| 文档资产 | docs、SVG、截图引用 | 文档口径一致，引用文件存在 |

## 完成定义

迁移只有在以下条件同时满足时才算完成：

- 源码和文档旧名扫描无残留。
- 文件名旧名扫描无残留。
- TypeScript build 通过。
- Rust tests 通过。
- whitespace check 通过。
- Atlas 静态入口资源可访问。
- 最终汇报明确说明数据断开策略和任何残余限制。
