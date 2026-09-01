# yunapi.rs 错误处理重构方案

> 分支: `fix/master-quality`
> 日期: 2026-09-01
> 状态: **已实施**（2026-09-01 验证通过，实际结果见第七节）

## 一、背景与动机

`src/yunapi.rs` 是库的核心网络层，当前存在多处 panic 风险和错误信息丢失问题：

| # | 位置 | 问题 | 后果 |
|---|---|---|---|
| 1 | `reqest()` 内 `serde_json::from_str(&text).unwrap()` | JSON 解析失败 | **panic** |
| 2 | 各方法 `self.reqest(...).unwrap()` | 网络请求失败 | **panic** |
| 3 | 各方法 `value["errno"].as_i64().unwrap()` | 百度响应格式异常 | **panic** |
| 4 | `get_file_dlink` 内 `link_vec[0]` | 响应 list 为空 | **越界 panic** |
| 5 | 错误分支写死 `"Get User infomation error."` 等文案 | 百度返回的 `errmsg` 被丢弃 | 用户拿不到真实失败原因 |
| 6 | `get_addr` 内 `to_string(params).unwrap_or_default()` | 参数序列化失败 | **静默丢弃参数**，发送错误请求 |
| 7 | 不检查 HTTP 状态码 | 4xx/5xx 也被当成功处理 | 必然解析失败，错误不可读 |
| 8 | `GetFileInfo` 节点使用 `http://` | 明文传输 | 安全隐患 |
| 9 | `get_files_info` 手写索引逐字段取值 | 与 `get_files_list` 的 serde 风格不一致 | 易漏字段、易 panic |
| 10 | 私有方法 `reqest` 拼写错误 | 代码质量 | 命名不规范 |
| 11 | `get_file_dlink` 内 `Err(ApiError::new(..., "Get file dlink error."))` | 内部真实错误被固定文案覆盖 | 百度 errmsg 丢失 |
| 12 | `search_with_key` 错误分支文案复制粘贴自 `get_files_info`（`"Get files info error."`） | 错误提示文不对题 | 误导调用方 |

## 二、重构目标

1. **消除所有 panic 路径**：网络、解析、格式异常一律返回 `Err(ApiError)`。
2. **透传百度 `errmsg`**：调用方拿到百度返回的真实错误信息。
3. **公共 API 零破坏**：所有 `pub fn` 签名与返回类型不变。
4. **统一解析风格**：list 解析全部走 serde，删除手写索引取值。

## 三、范围边界

- **本次改动**：`src/yunapi.rs` + `src/models.rs`（仅 `FileInfoEx` 增加 derive）。
- **本次不改**（另开任务跟踪）：
  - `src/util.rs` 的 `download()` 存在一整条 unwrap 链（`send()` / `bytes()` / `write_all()` / 文件 `open()` / `CONTENT_LENGTH` 解析等），与本次"消除 panic"同主题，但它是 `pub fn ... -> ()`，改动必然破坏公共 API 签名，故**另开任务**；
  - `YunApi::new` 的 reqwest client 未设置超时（`blocking::Client::new()` 默认无 timeout），网络挂起时调用会无限阻塞，建议后续任务统一配置超时（集成测试 `api_tests.rs` 同样受益）；
  - `YunFs` 的 `to_str().unwrap()` 路径为理论不可达 panic（路径均来自字符串拼接，不会非 UTF-8），暂不动。
- `src/error.rs` 无需改动（`ApiError` 已支持 errno + custom prompt 组合）。

## 四、方案设计（三层）

### 第 1 层：私有基础设施

- `reqest` 改名 `request`（私有方法，无外部影响）。
- `get_addr` 返回类型改为 `Result<String, ApiError>`：参数序列化失败返回 Err，不再 `unwrap_or_default()` 静默丢弃。
- `request` 内：
  - `send()` 失败 → `map_err`（错误信息包含底层请求错误）；
  - 检查 HTTP 状态码：非 2xx 时**先尝试解析响应 body 的 `errno` / `errmsg`**，解析成功则透传百度真实错误（`ApiError::new(errno, errmsg)`，errmsg 缺失 fallback `"no errmsg from baidu"`）；解析失败才回退 `ApiError::from("HTTP status {code} from baidu api")`（内部错误码 8989）；
  - `.text()` 失败 → `map_err`；
  - `serde_json::from_str` 失败 → `map_err`（错误信息含解析错误详情）。

### 第 2 层：响应校验辅助（新增两个私有方法）

```rust
/// 检查 errno 是否为 0;不为 0 则返回透传百度 errmsg 的错误
fn check_errno(value: &Value) -> Result<(), ApiError>

/// 把响应 "list" 字段解析为 Vec<T>,统一错误处理
fn parse_list<T: DeserializeOwned>(value: &Value) -> Result<Vec<T>, ApiError>
```

- `check_errno`：`errno` 缺失 → 内部错误（8989）；`errno != 0` → `ApiError::new(errno, errmsg)`，`errmsg` 缺失时 fallback 为 `"no errmsg from baidu"`。
- `parse_list`：`list` 字段缺失或非数组 → 内部错误；逐项 serde 解析，失败项返回带具体原因的内部错误；空列表返回 `Ok(vec![])`（空目录是合法响应）。

### 第 3 层：公共 API 方法（签名不变）

每个方法从（以 `get_user_info` 为例）：

```rust
let value = self.reqest(YunNode::GetUserInfo, &params).unwrap();
let error = value["errno"].as_i64().unwrap();
if error == 0 {
    Ok(serde_json::from_value(value).unwrap())
} else {
    Err(ApiError::new(error, "Get User infomation error."))
}
```

变为：

```rust
let value = self.request(YunNode::GetUserInfo, &EmptyParams)?;
Self::check_errno(&value)?;
serde_json::from_value(value).map_err(|e| ApiError::from(&format!("malformed user info: {}", e)))
```

涉及方法：`get_user_info`、`get_quota_info`、`get_files_info`、`get_files_list`、`get_files_dlink_vec`、`get_file_dlink`、`search_with_key`。

`get_files_dlink_vec` 的 dlink 提取仍为逐项取值，但改为 `ok_or_else` 返回 Err（非 panic）。

`get_file_dlink` 改为**直接透传内部错误**（`?`），不再包装成固定文案（修复问题 #11）；空列表用 `pop().ok_or_else(...)` 返回 Err（非 panic，修复问题 #4）：

```rust
let mut link_vec = self.get_files_dlink_vec(&file_vec)?;
link_vec.pop().ok_or_else(|| ApiError::from("empty dlink list"))
```

## 五、具体改动点

| 文件 | 改动 |
|---|---|
| `src/yunapi.rs` | 上述三层重构；`get_node_addr` 中 `GetFileInfo` 由 `http://` 改 `https://`；`search_with_key` 错误文案随 errmsg 透传自然修复（问题 #12） |
| `src/models.rs` | `FileInfoEx` 增加 `Serialize, Deserialize, Debug, Clone` derive（供 `parse_list` 使用，顺带补齐与其他模型的一致性） |
| `src/yunapi.rs`（模块测试） | `get_addr` 测试适配 `Result` 返回值（`.unwrap()`）；新增 `check_errno` 三个测试（errno=0 / 非0+errmsg / errno 缺失）；新增 `parse_list` 三个测试（正常解析 / list 缺失 / 单项解析失败） |
| `src/error.rs` | 无需改动（见范围边界） |

## 六、兼容性影响

- **公共 API 签名完全不变**，调用方代码零修改。
- 行为变化：原先 panic 的场景现在返回 `Err` —— 对正常调用方是纯改进。
- 错误文案变化：写死的 `"Get User infomation error."` 等被百度 `errmsg` 取代（errmsg 缺失时 fallback）。

## 七、验证计划

1. `cargo build` —— 编译通过；
2. `cargo test` —— 全部通过（单元数以重构后实际输出为准：原 39 单元 = error 8 + yunapi 5 + util 16 + api_tests 10[ignored]，新增 check_errno 3 + parse_list 3）；
3. `cargo test --doc` —— 全绿（6 doctest）；
4. 新增 `check_errno` / `parse_list` 测试共 6 个，覆盖全部分支；
5. HTTP 状态码分支无法纯构造测试，由 `src/tests/api_tests.rs` 的 `error_key` 集成测试（无效 token 返回 Err 而非 panic）间接覆盖。

**实际结果（2026-09-01）**：

- `cargo build` —— 无警告无错误；
- `cargo test --lib` —— 50 通过 / 0 失败 / 9 ignored（其中 10 个为本重构新增：get_addr 序列化失败 1 + parse_response 4 + check_errno 3 + parse_list 3）；
- `cargo test --doc` —— 6 doctest 全绿。

> 实施说明：`request` 的错误处理逻辑按讨论抽出为私有纯函数 `parse_response(status, text)`（见 TDD 计划），行为与文档第四节一致，便于离线单测。

## 八、决策记录（2026-09-01 已全部确认）

1. `FileInfoEx` 加 `Serialize, Deserialize, Debug, Clone` derive（顺带一致性补齐）—— 已确认。
2. errmsg 透传格式：`ApiError::new(errno, errmsg)`，errmsg 缺失 fallback `"no errmsg from baidu"` —— 已确认。
3. HTTP 非 2xx：**先尝试解析 body 的 `errno`/`errmsg` 透传真实错误**，解析失败才回退 8989 + `"HTTP status {code}"` —— 已确认。
4. `get_file_dlink` 空列表返回 `Err("empty dlink list")` 而非 panic；内部错误直接透传，不再包装成固定文案 —— 已确认。
5. 范围：util.rs 的 `download()` unwrap 链不在本次范围，另开任务 —— 已确认。
6. 测试策略：纯构造 `Value` 测试，不引入 mock server 依赖 —— 已确认。

## 九、实施步骤

1. `src/models.rs`：FileInfoEx 加 derive；
2. `src/yunapi.rs`：按三层重构重写 impl（含模块测试适配与新增测试）；
3. 运行验证计划全项；
4. 提交到 `fix/master-quality` 分支（提交信息见 README 惯例）。
