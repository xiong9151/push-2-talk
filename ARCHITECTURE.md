# PushToTalk 架构索引

## 这份文档是什么

`ARCHITECTURE.md` 现在只承担一件事：

- 作为系统架构的 **导航页 / index**

详细内容已经拆到 `docs/architecture/` 下，按“先总后分、按需下钻”的方式组织，避免每次都把整份系统说明塞进上下文。

## 怎么读

按你的当前任务选择入口：

- 想先看全局：[`docs/architecture/system-overview.md`](docs/architecture/system-overview.md)
- 只改前端：[`docs/architecture/frontend.md`](docs/architecture/frontend.md)
- 只改后端：[`docs/architecture/backend/overview.md`](docs/architecture/backend/overview.md)
- 追听写链路：[`docs/architecture/flows/dictation.md`](docs/architecture/flows/dictation.md)
- 追助手/学习链路：[`docs/architecture/flows/assistant-and-learning.md`](docs/architecture/flows/assistant-and-learning.md)
- 查前后端契约：[`docs/architecture/contracts.md`](docs/architecture/contracts.md)
- 查配置、持久化、平台约束：[`docs/architecture/persistence-and-platform.md`](docs/architecture/persistence-and-platform.md)
- 查高风险改动区：[`docs/architecture/risk-zones.md`](docs/architecture/risk-zones.md)

## 文档树

```text
ARCHITECTURE.md
└── docs/architecture/
    ├── system-overview.md
    ├── frontend.md
    ├── contracts.md
    ├── persistence-and-platform.md
    ├── risk-zones.md
    ├── backend/
    │   ├── overview.md
    │   ├── asr-and-pipeline.md
    │   └── system-integration.md
    └── flows/
        ├── dictation.md
        └── assistant-and-learning.md
```

## 建议阅读路径

### 路径 1：第一次进仓

1. [`README.md`](README.md)
2. [`docs/architecture/system-overview.md`](docs/architecture/system-overview.md)
3. [`docs/architecture/backend/overview.md`](docs/architecture/backend/overview.md)
4. [`docs/architecture/contracts.md`](docs/architecture/contracts.md)

### 路径 2：前端改动

1. [`docs/architecture/frontend.md`](docs/architecture/frontend.md)
2. [`docs/architecture/contracts.md`](docs/architecture/contracts.md)
3. 需要追链路时再看对应 `flows/`

### 路径 3：后端改动

1. [`docs/architecture/backend/overview.md`](docs/architecture/backend/overview.md)
2. [`docs/architecture/backend/asr-and-pipeline.md`](docs/architecture/backend/asr-and-pipeline.md) 或 [`docs/architecture/backend/system-integration.md`](docs/architecture/backend/system-integration.md)
3. [`docs/architecture/persistence-and-platform.md`](docs/architecture/persistence-and-platform.md)

### 路径 4：排查问题

1. 先看对应业务流文档
2. 再看 [`docs/architecture/contracts.md`](docs/architecture/contracts.md)
3. 最后看 [`docs/architecture/risk-zones.md`](docs/architecture/risk-zones.md)

## 设计原则

这套结构遵循渐进式披露：

- 根索引只给方向，不堆细节
- 二级文档讲一个主题，不混多个关注点
- 三级文档只展开高复杂度区域，比如后端子系统和核心链路

如果后续某个专题继续膨胀，就继续往下拆，不把根索引重新写胖。
