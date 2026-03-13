# favorite_commands 双向同步设计方案（EasyFF <-> easyffweb）

## 1. 背景与目标

当前状态：
- EasyFF 已接入 easyffweb 登录鉴权。
- 本地 `favorite_commands` 仅本地增删查，无云端同步。

目标：
- 支持双向同步（本地改动推送到云端，云端改动拉取到本地）。
- 支持多客户端（同账号在多台设备）最终一致。
- 支持离线编辑后恢复网络自动补偿同步。
- 接口风格尽量对齐 `src/app/api/task/history/{list|upsert|delete}`。

非目标（本期不做）：
- 实时 WebSocket 推送（先做轮询/触发式增量同步）。
- 复杂 CRDT 合并（先用 LWW + 软删除墓碑）。

---

## 2. 数据模型设计

### 2.1 云端表（easyffweb）

建议新增：`favorite_commands`

字段：
- `id` TEXT PK（客户端生成，保持跨端稳定）
- `user_id` TEXT NOT NULL
- `title` TEXT NOT NULL
- `description` TEXT NULL
- `command` TEXT NOT NULL
- `created_at` BIGINT NOT NULL（客户端初始创建时间）
- `updated_at` BIGINT NOT NULL（逻辑更新时间，冲突判定主字段）
- `deleted_at` BIGINT NULL（软删除墓碑）
- `updated_by_device_id` TEXT NULL
- `version` BIGINT NOT NULL DEFAULT 1（服务端自增版本）

索引建议：
- `(user_id, updated_at DESC)`
- `(user_id, version DESC)`
- `(user_id, deleted_at)`

说明：
- 多端同步要保留墓碑（`deleted_at`），否则 A 端删除后 B 端可能“复活”。

### 2.2 本地表（EasyFF / SQLite）

当前本地字段：
- `id, title, description, command, created_at, updated_at`

建议扩展：
- `deleted_at INTEGER NULL DEFAULT NULL`
- `sync_state TEXT NOT NULL DEFAULT 'synced'`（`pending_upsert | pending_delete | synced | conflict`）
- `server_version INTEGER NULL`
- `updated_by_device_id TEXT NULL`

说明：
- 本地删除改为软删除（写 `deleted_at` + `sync_state=pending_delete`），同步成功后可按策略清理。

---

## 3. 接口设计（参考 task/history 风格）

统一前缀建议：`/api/app/favorite/commands/*`

鉴权策略：
- 与 `task/history` 一致，优先 `getUserInfo(req)`。
- 未登录不允许云同步（返回 `no auth, please sign in`）。

### 3.1 列表接口（兼容管理页展示）

`POST /api/app/favorite/commands/list`

请求：
- `page?: number`
- `limit?: number`
- `keyword?: string`
- `include_deleted?: boolean`

响应：
- `list: FavoriteCommandDTO[]`
- `total: number`

### 3.2 Upsert 接口（推送本地新增/修改）

`POST /api/app/favorite/commands/upsert`

请求：
- `device_id: string`
- `items: Array<{
  id: string;
  title: string;
  description?: string;
  command: string;
  created_at: number;
  updated_at: number;
  deleted_at?: number | null;
  base_version?: number | null;
}>`

服务端行为：
- 以 `(user_id, id)` 查记录。
- 冲突规则：`updated_at` 更大者胜（LWW）。
- 若相等：`device_id` 字典序较大者胜（稳定 tie-breaker）。
- 写入后 `version = version + 1`。

响应：
- `accepted: Array<{id, version, updated_at, deleted_at}>`
- `rejected?: Array<{id, reason, server_record}>`（可选）

### 3.3 Delete 接口（语义删除）

`POST /api/app/favorite/commands/delete`

请求：
- `device_id: string`
- `ids?: string[]`
- `id?: string`
- `deleted_at?: number`（默认服务端当前时间）

服务端行为：
- 不做物理删，更新 `deleted_at/updated_at/version`。

### 3.4 增量拉取接口（多端同步关键）

`POST /api/app/favorite/commands/changes`

请求：
- `cursor?: number`（上次拉取到的 `version`）
- `limit?: number`（默认 200）

响应：
- `changes: FavoriteCommandDTO[]`（包含删除墓碑）
- `next_cursor: number`
- `has_more: boolean`

说明：
- `changes` 是“版本增量流”，用于多客户端最终一致。

---

## 4. 客户端同步策略（EasyFF）

## 4.1 触发时机

触发一次完整同步（push -> pull）：
- 用户登录成功后。
- 应用启动且已登录时。
- 新增/编辑/删除收藏后（可 1~3 秒防抖合并）。
- 网络从离线恢复在线时。

## 4.2 同步流程（推荐）

1. `pushPending()`
- 上传本地 `sync_state in (pending_upsert, pending_delete)` 记录到 `upsert/delete`。
- 服务端成功后回写本地 `server_version`，并标记 `synced`。

2. `pullChanges(cursor)`
- 循环调用 `changes` 直到 `has_more=false`。
- 将变更应用到本地（含墓碑）。
- 更新本地 `sync_cursor=next_cursor`。

3. 冲突处理
- 默认自动 LWW。
- 若本地被覆盖且业务需要提示，可把记录标记为 `conflict` 并提示用户。

## 4.3 本地应用变更规则

收到远端记录时：
- 若本地不存在：直接插入。
- 若远端 `updated_at` 更新：覆盖本地。
- 若远端 `deleted_at != null`：本地标记删除（或从列表隐藏）。
- 若本地为 `pending_*` 且本地 `updated_at` 更大：保留本地，等待下一次 push。

---

## 5. 后端实现建议（easyffweb）

目录建议：
- `src/app/api/favorite/commands/list/route.ts`
- `src/app/api/favorite/commands/upsert/route.ts`
- `src/app/api/favorite/commands/delete/route.ts`
- `src/app/api/favorite/commands/changes/route.ts`

风格对齐 `task/history`：
- 统一 `OPTIONS` + `withCors`
- 统一 `respData / respOk / respErr`
- 统一 `getUserInfo(req)` 鉴权

模型层建议：
- `src/shared/models/favorite_commands.ts`
  - `getFavoriteCommandsList`
  - `upsertFavoriteCommandsBatch`
  - `softDeleteFavoriteCommands`
  - `getFavoriteCommandChangesByVersion`

---

## 6. EasyFF 改造建议（当前项目）

## 6.1 Tauri 存储层

扩展 `src-tauri/src/storage/favorite_commands.rs`：
- 新增字段：`deleted_at/sync_state/server_version/updated_by_device_id`
- 新增方法：
  - `list_pending_sync()`
  - `mark_synced(batch_result)`
  - `apply_remote_changes(changes)`
  - `set_sync_cursor/get_sync_cursor`

## 6.2 命令层

新增同步命令（Tauri command）：
- `favorite_sync_push()`
- `favorite_sync_pull()`
- `favorite_sync_all()`

## 6.3 前端 store 层

在 favorite store 增加：
- `syncNow()`
- `scheduleSync()`（防抖）
- `lastSyncAt/syncing/syncError`

---

## 7. 一致性与容错

- 幂等：`upsert/delete` 以 `(user_id,id)` 幂等处理。
- 断网：本地保留 `pending_*`，恢复网络后补偿。
- 重试：指数退避（1s/2s/4s，最多 5 次）。
- 安全：所有接口必须 user 维度隔离。

---

## 8. 分阶段落地计划

Phase 1（最小可用）
- 后端：`list/upsert/delete`
- 客户端：仅 push（单向）

Phase 2（双向同步）
- 后端：`changes(cursor)`
- 客户端：push + pull + cursor

Phase 3（多端优化）
- 冲突提示 UI
- 后台定时同步
- 墓碑清理任务（例如 30 天后归档/清理）

---

## 9. 请求/响应示例

### upsert 请求
```json
{
  "device_id": "dev-abc-001",
  "items": [
    {
      "id": "fav-1730000000000-1",
      "title": "压缩视频(H.264)",
      "description": "常规压缩",
      "command": "ffmpeg -i input.mp4 -c:v libx264 -crf 23 output.mp4",
      "created_at": 1730000000000,
      "updated_at": 1730000001000,
      "deleted_at": null,
      "base_version": 12
    }
  ]
}
```

### changes 请求
```json
{
  "cursor": 120,
  "limit": 200
}
```

### changes 响应
```json
{
  "changes": [
    {
      "id": "fav-1730000000000-1",
      "title": "压缩视频(H.264)",
      "description": "常规压缩",
      "command": "ffmpeg -i input.mp4 -c:v libx264 -crf 22 output.mp4",
      "created_at": 1730000000000,
      "updated_at": 1730000100000,
      "deleted_at": null,
      "version": 121,
      "updated_by_device_id": "dev-xyz-002"
    }
  ],
  "next_cursor": 121,
  "has_more": false
}
```

---

## 10. 验收标准

- A 端新增收藏，B 端 10 秒内可见（触发同步后）。
- A 端删除收藏，B 端不会“复活”。
- A/B 同时修改同一记录，最终按 LWW 收敛，且不报错。
- 离线修改后恢复网络，可自动补偿同步成功。
