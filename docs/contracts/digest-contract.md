# Digest Contract

`DigestRecord` 表示一次完整 closure 的压缩归档。

## 最小字段

- `digest_id`
- `closure_id`
- `session_id?`
- `task_id?`
- `topic_thread_id?`
- `summary`
- `continuity_tail[]`
- `note_refs[]`
- `artifact_candidates[]`
- `created_at`

## 契约要求

1. 每个完整 closure 必须生成一个 digest。
2. interrupted segment 不单独形成 final digest。
3. digest 用于 context rebuild、history continuity、知识再利用。