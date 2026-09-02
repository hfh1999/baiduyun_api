# HTTP 后端切换与异步能力方案(同步换 ureq,异步用 reqwest)

> 分支: `feat/sync-and-async`
> 日期: 2026-09-02
> 状态: **已实施**(2026-09-02,实际结果见第六节末)
> 版本背景: 0.3.0 已发布,本次**公共 API 零破坏**(纯内部实现更换)

## 一、背景与动机

### 1.1 现状问题

`baiduyun_api` 0.3.0 用 `reqwest` 的 **blocking** 模式(依赖树中 reqwest 的 blocking 壳内部自带 tokio + hyper 全家桶):

| 问题 | 说明 |
|---|---|
| "轻量"名不副实 | 同步用户被迫编译 tokio/hyper 大量传递依赖,依赖树约百级 |
| 异步路线悬而未决 | 讨论过的"异步核心 + block_on 壳"方案让同步用户继续背 tokio,收益有限 |
| 无真异步 API | 服务端场景(axum/tokio 生态)用户被排除 |

### 1.2 决策演进(讨论记录)

1. **方案一(reqwest 双模式)**:异步核心 + block_on 同步壳。缺点:同步用户依旧编译 tokio;"runtime within runtime" 坑需要特殊处理。
2. **方案二(双后端 feature 切换)** ✅:同步用 **ureq**(纯同步,真轻量),异步用 **reqwest**(未来按需编译)。sync 用户依赖树只到 ureq 级,异步用户才付 reqwest+tokio 成本;feature additive 无冲突(两个类型 `YunApi`/`YunApiAsync`,无方法名冲突)。
3. 否决"永远只写同步"(放弃异步生态)与"保持现状"(轻量名不副实)。

**关键收益**:
- sync 用户真轻量(依赖树: ureq + rustls + serde_json 级)
- ureq 纯同步 → 无 runtime → **block_on 的坑(cannot start a runtime from within a runtime)彻底不存在**
- 异步能力成为按需 feature,不必现在支付维护成本
- Windows 下 ureq 默认 rustls(纯 Rust),无需 OpenSSL 环境

## 二、实验结论(2026-09-02,真实百度接口)

临时实验项目 `%TEMP%/baiduyun_ureq_exp`(ureq 3.4.0),token 来自项目 `.env`:

| # | 实验 | 结果 |
|---|---|---|
| 1 | `list` GET 请求(pan.baidu.com, 带 access_token query) | ✅ 200, errno=0, 10 条 |
| 2 | `locateupload` 获取上传域名 | ✅ 200, 正常返回 servers |
| 3 | `upload` multipart 上传真实文件(26B 文本) | ✅ 200, 返回 path/md5/fs_id |
| 4 | `filemanager delete` 清理远端文件 | ✅ 200, errno=0 |

**实测确认的 ureq 3.4 API 事实**:
- 构建: `ureq::Agent::config_builder().http_status_as_error(false).build()` → `let agent: ureq::Agent = config.into()`
- `http_status_as_error(false)` 关闭非 2xx 报错(我们的 `parse_response` 自行解析 status+body,与 reqwest 行为对齐)
- multipart: `ureq::unversioned::multipart::Form::new().file("file", path)?` → `agent.post(url).send(form)`(自动设置 multipart Content-Type)
- **`Form::file` 是流式的**(内部 `std::fs::File` → `SendBody::from_file`),2GB 单步上传不读进内存 ✅
- 响应: `resp.status()` 返回 `ureq::http::StatusCode`(http crate 类型,`.into()` 转 u16);`resp.body_mut().read_to_string()?`
- 无 `send_string`;文本 body 用 `.send(String)`(实现 `AsSendBody`),urlencoded 需显式 `Content-Type: application/x-www-form-urlencoded`
- 上传成功响应**无 error_code 字段**(JSON 里为 null),与现有 `unwrap_or(0)` 判断兼容

**风险记录**: multipart 位于 `ureq::unversioned` 模块——**不保证 semver**,ureq 升级时该 API 可能变化(实验用的 3.4.0 验证通过,升级需回归实测)。

## 三、目标架构

### 3.1 依赖与 features

```toml
[features]
default = ["sync"]
sync = ["dep:ureq"]                      # 同步后端(默认)
async = ["dep:reqwest", "dep:tokio"]     # 异步后端(本次不做,图纸见第七节)

[dependencies]
ureq = { version = "=3.4.0", features = ["multipart"], optional = true }   # =3.4.0: 精确锁死,防 unversioned::multipart 破坏(见第八节)
reqwest = { version = "0.11.4", features = ["json", "cookies", "multipart"], optional = true }
tokio = { version = "1", features = ["rt"], optional = true }
```

- 去掉 reqwest 的 `blocking` feature(同步侧不再用 reqwest)
- 两个 feature 可同时启用(类型隔离,无冲突);`sync` 默认开,现有用户零感知
- 本次实施**只做 sync 后端迁移**,`async` feature 定义留到第七节实施

### 3.2 共享层解耦(先决改造)

`parse_response` 的签名从 `reqwest::StatusCode` 改为 **`u16`**(纯数字,不依赖任何 client 类型):

```rust
fn parse_response(status: u16, text: String) -> Result<Value, ApiError>
```

- 现有单元测试里 `reqwest::StatusCode::OK` / `BAD_REQUEST` 等改为 `200` / `400` 字面量
- 这是双后端方案的前提:共享层与 HTTP 栈彻底解耦

### 3.3 后端请求层(两侧对称:直接持有 client,不引入 Backend 封装)

**形态决策**:同步侧不引入额外 `Backend` 模块,与现状结构保持一致——HTTP 客户端直接作为 API 结构体字段,请求层为 `impl` 内私有方法。异步侧(未来)采用**完全相同的形状**:

```rust
// 同步(本次实施)
pub struct YunApi {
    access_token: String,
    agent: ureq::Agent,   // 原 reqwest::blocking::Client → ureq::Agent
}

// 异步(未来,第七节)
pub struct YunApiAsync {
    access_token: String,
    client: reqwest::Client,
}
```

两侧对称的理由:

- **对称性**:同为"API 结构直接持有 client + impl 内私有请求层",心智模型统一,无"一侧封装一侧直持"的不和谐;
- **最小 diff**:现有 `request_get`/`request_post`/`upload` 本就是 `YunApi` 私有方法,迁移 = 原地改方法体内部实现;
- **无复用需求**:请求层只有本 API 类型一个调用方,独立模块不产生价值;
- **测试已覆盖**:19 个网络测试直接打请求层,无需为封装层单独 mock。

请求层差异(每侧约 60 行)归属各自 `impl`;共享的是模块级纯函数层(`parse_response`/`parse_list`/`check_errno`/`get_node_addr`/参数组装)。

### 3.4 公共方法体零改动(关键路径)

现有 13 个公共方法体只依赖私有层 `request_get` / `request_post` / `filemanager` / `get_upload_host` 的**签名**(均返回 `Result<Value, ApiError>`)。只要私有层签名不变,公共方法体**一行不改**:

| 私有层 | 改造(均在 `impl YunApi` 内原地改,方法结构不动) |
|---|---|
| `request_get` | `blocking::Client` → `agent.get(&addr)`,读 body 后 `parse_response(status.into(), text)` |
| `request_post` | → `agent.post(...)` + 显式 form Content-Type,body 为序列化好的字符串(form 序列化逻辑不变) |
| `parse_http_response` | 删除(reqwest 类型);错误映射在发送处内联 |
| `upload` 的 multipart | `reqwest::blocking::multipart::Form` → `ureq::unversioned::multipart::Form::new().file(...)`,经 `agent.post().send(form)` |

## 四、兼容性影响

- **公共 API 零变化**:`YunApi` 所有方法签名、返回类型、错误行为不变
- 错误文案微调:reqwest 的 `send request error: {e}` → ureq 错误 Display(格式略不同,`ApiError::new(8989, ...)` 不变)
- 依赖变化:`reqwest`(可选) + `ureq`(默认)——用户 Cargo.lock 变化,编译产物变小
- TLS 栈变化:reqwest native-tls(schannel) → ureq rustls(Mozilla 根证书)——标准 CA 站点无感知;企业内部自签 CA 场景需自定义 `tls_config`(文档注明)

## 五、测试计划

### 离线测试(已有,零改动预期)

- `src/yunapi.rs` 单元测试:仅 `test_parse_response_*` 系列改 `u16` 字面量
- `src/util.rs` / `src/models.rs` 测试:不动
- 预期 `cargo test --lib` 全绿(65+)

### 网络测试(全量回归)

- 现有 19 个(api_tests.rs)全部重跑,验证行为一致:
  - 错误路径(`error_key` errno=-6、上传重复 31061、mkdir 重复 -8)
  - 写路径 roundtrip(mkdir/remove/mv/cp/rename/upload 自清理)
  - YunFs 相对路径
- 注意:ureq 的错误映射在 `map_err` 文案上与 reqwest 不同,不影响 errno 断言

## 六、开发步骤(TDD)

### 阶段 1:共享层解耦 + 后端模块骨架

| 步骤 | 内容 | 验证 |
|---|---|---|
| 1.1 | `parse_response` 改 `u16` + 单元测试字面量 | lib 测试全绿 |
| 1.2 | `YunApi` 字段 `blocking::Client` → `ureq::Agent`(配置 `http_status_as_error(false)`) | 编译 |
| 1.3 | 迁移 `request_get` 走 ureq(先让 list 走通),其余私有层暂不动 | 编译 + `test_api` 网络测试 |
| 1.4 | **提交**:`内部修改: HTTP后端迁移准备,共享层解耦+ureq骨架` | — |

### 阶段 2:全量迁移

| 步骤 | 内容 | 验证 |
|---|---|---|
| 2.1 | `request_post` 迁 ureq POST(filemanager/create) | 网络测试 mkdir/mv/cp/rename |
| 2.2 | `get_upload_host` + `upload` 迁 ureq multipart | 网络测试 upload roundtrip |
| 2.3 | 删除 reqwest blocking 依赖,清理无用 feature | `cargo tree` 确认无 reqwest |
| 2.4 | **提交**:`内部修改: 同步后端迁移至ureq,依赖树瘦身` | — |

### 阶段 3:回归与文档

| 步骤 | 内容 |
|---|---|
| 3.1 | 全量网络测试回归(19 个) |
| 3.2 | README / lib.rs 文档:依赖说明(默认仅 ureq)、TLS 差异注明 |
| 3.3 | 本文档状态更新为"已实施",记录实测 |
| 3.4 | **提交** |

**实际结果(2026-09-02)**:

- 实施压缩为两个提交:`3603797`(parse_response 解耦 u16)、`ecb9a41`(ureq 全量迁移)——字段与发送层强耦合(所有方法共用同一 client 字段),无法按文档设想只迁 request_get 后停,故 1.2/1.3 与阶段 2 一次完成;
- 阶段 1.1 严格走 RED(单测改 u16 字面量 → 5 处编译错误)→ GREEN(lib 65 通过 / 0 失败);
- `cargo test`:lib 65 + doc 9 全绿;**网络测试 19/19 通过**(27s 单线程,含写操作 roundtrip/上传/YunFs 相对路径/错误透传),行为与 reqwest 时代完全一致;
- 依赖树瘦身:直接依赖仅剩 4 个(ureq =3.4.0 / serde / serde_json / serde_urlencoded),reqwest/tokio/hyper 全部移除;
- **实施偏差**:`util::download`(分段下载工具)也使用 reqwest blocking,随迁移一并改写(文档正文未列,遗漏项);`Cargo.lock` 被 gitignore,提交不含 lock;
- 需求变更记录:第八节风险 1 的版本策略最终定为**精确锁死 `ureq = "=3.4.0"`**(用户决策:极端稳妥,升级必须人工回归实测)。

## 七、异步后端图纸(本次不做,后续单独实施)

- 新增 `src/api_async.rs`:`YunApiAsync`(reqwest 原生异步 Client)+ 13 个 `async fn`
- 共享层(models/parse_response/check_errno/parse_list/参数组装)直接复用
- 方法体与同步版同构(组装参数 → `.await?` → 解析),接受少量复制(约 300 行)
- `async` feature 引入 tokio;不强制用户 runtime(纯 `std::future`)
- 网络测试:async 版只跑核心子集,规避百度频控(31034)
- 发布节奏:作为 0.4.0 内容,若先发布则同步侧迁移独立成 0.3.x

## 八、已知风险与决策点

1. **`unversioned::multipart` 不稳定**:ureq 官方声明该 API 不保证 semver。**对策:精确锁定 `ureq = "=3.4.0"`**(连补丁版本也不放行,实验验证的版本永不漂移);代价是拿不到 3.4.x 的 bug 修复——升级到任何新版本前,必须人工回归实测(实验 3 upload 用例 + 网络测试 upload roundtrip),通过后再提升版本号。
2. **TLS 栈差异**:rustls(Mozilla 根) vs schannel——标准场景无影响;自签 CA 用户需 `tls_config`(文档注明,不内置配置项)。
3. **cookies feature 清理**:现有 Cargo.toml 的 `cookies` 未使用,随迁移移除。
4. **`async` feature 的时机**:等真实需求信号(用户提出 / 进军服务端场景)再实施第七节;图纸已完备,不预先支付双 API 维护成本。
5. **百度频控**:网络测试双后端双跑会翻倍请求,async 测试设计为子集。
