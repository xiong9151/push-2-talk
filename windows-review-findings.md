我已完成对以下两个文件的审查：

## OverlayWindow.tsx (d:\git\push-2-talk\src\windows\OverlayWindow.tsx)

### 1. Tauri listen() cleanup

**useSmoothAudioLevel (第40-58行):** 正确。有 `cancelled` 标志 + `unlisten` 变量 + async setup 的竞态保护（若 `listen()` resolve 后 `cancelled` 为 true，立即调用返回的 `u()` 解除注册）+ cleanup 函数中再次调用 `unlisten()`。

**主事件监听 (第351-443行):** 正确。使用 `unlistenFns: UnlistenFn[]` 数组 + `cancelled` 标志。辅助函数 `registerListener` 正确处理竞态条件（cancelled 后立即解注册并返回 false）。cleanup 函数清除所有 listener。

**结论：** OK，没有 cleanup 泄漏。

### 2. handleFinish/handleCancel 成功路径是否重置 isSubmitting

**handleFinish (第477-492行):** 正确。成功路径 `setIsSubmitting(false)` 在第487行（try 块末尾），失败路径在第490行（catch 块末尾）。

**handleCancel (第494-509行):** 正确。成功路径 `setIsSubmitting(false)` 在第504行（try 块末尾），失败路径在第507行（catch 块末尾）。

**结论：** OK，两个处理器都在成功路径重置 isSubmitting。

### 3. pointerEvents 条件

**第523行:** `style={status === "transcribing" ? { pointerEvents: 'none' } : undefined}`。转写中时 overlay 不拦截点击；results 模式时可操作结果列表。**正确。**

### 4. debounce 定时器 cleanup

- `submitDebounceRef`: handleFinish (481-484行) 和 handleCancel (498-501行) 使用，cleanup effect (512-518行) 清除。**正确。**
- auto-focus timeout (301-308行): cleanup `clearTimeout`。**正确。**
- 15秒转写超时 (446-460行): cleanup `clearTimeout`。**正确。**
- 60秒锁定录音超时 (462-475行): cleanup `clearTimeout`。**正确。**

### 5. useEffect 依赖数组

所有 effect 依赖数组均正确，没有闭包过期问题。`statusRef` 的 ref 模式（第272-273行）确保了主 listener effect（空数组依赖）中能读取到最新的 status 值。

**结论：** OK。

### 额外发现的问题

**Bug：confirmResult 缺少 isSubmitting 防护 (第286行)**

`confirmResult` 没有像 `handleFinish`/`handleCancel` 那样在开头添加 `if (isSubmitting) return;` 防护。由于 `useCallback` 的依赖数组为 `[]`，闭包中捕获的 `isSubmitting` 始终为初始值 `false`。这意味着：
- 用户快速点击两个不同的结果项时，会触发两次并发的 `invoke("select_transcription_result", ...)` 调用
- 后端可能会收到两个插入请求，导致文本被插入两次

```typescript
// handleFinish 有防护 (第478行)
const handleFinish = async () => {
    if (isSubmitting) return;  // 有防护
    setIsSubmitting(true);
    ...

// confirmResult 缺少防护 (第286行)
const confirmResult = useCallback(async (item: TranscriptionResultItem) => {
    setIsSubmitting(true);  // 无防护，直接执行
    ...
```

**修复建议:** 在 `confirmResult` 开头添加 `if (isSubmitting) return;`。

---

## NotificationWindow.tsx (d:\git\push-2-talk\src\windows\NotificationWindow.tsx)

### 1. Tauri listen() cleanup

**第40-78行:** 正确。有 `cancelled` 标志 + `unlisten` 变量 + async setup 的竞态保护 + cleanup 函数中解注册。依赖数组 `[addSuggestion]` 中 `addSuggestion` 是稳定的（useCallback 空依赖），因此 effect 只执行一次。

### 2. handleFinish/handleCancel — 不适用，该组件没有这些处理函数。

### 3. pointerEvents 条件

**第90-106行:** 外层容器 `pointer-events-none`（点击穿透），内层通知卡片容器 `pointer-events-auto`（可点击）。这是通知浮层的正确模式。

### 4. debounce 定时器 cleanup — 不适用，该组件没有使用 debounce 定时器。

### 5. useEffect 依赖数组

- 主监听 effect: `[addSuggestion]` — 正确（addSuggestion 稳定）
- 自动隐藏 effect: `[suggestions.length]` — 正确

**结论：** OK，没有 bugs。

---

## VocabularyLearningToast.tsx (从 NotificationWindow 引用的组件，纳入审查范围)

### 发现的问题

**Bug 1：handleAdd 成功路径未重置 isSubmitting (第24-40行)**

成功路径（`invoke("add_learned_word")` 完成后）没有调用 `setIsSubmitting(false)`，仅在 catch 块中重置。这意味着添加成功后，在 300ms 退出动画期间，"添加"按钮仍处于禁用状态。

```typescript
const handleAdd = useCallback(async () => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    try {
      await invoke("add_learned_word", { word: suggestion.word, source: "auto" });
      setIsExiting(true);
      setTimeout(onAdd, 300);
      // 缺少 setIsSubmitting(false) ←---
    } catch (error) {
      console.error("添加词汇失败:", error);
      setIsSubmitting(false);  // 仅在失败时重置
    }
  }, [suggestion.word, onAdd, isSubmitting]);
```

实际上此 bug 被轻量化了，因为 300ms 后 `onAdd` 触发组件卸载。但原理上仍是一个状态管理缺口。

**修复建议:** 在 `setTimeout(onAdd, 300);` 后添加 `setIsSubmitting(false);`。

**Bug 2：在 setCountdown 状态更新器内调用副作用 (第59-66行)**

`invoke()`、`setIsExiting()` 和 `setTimeout()` 被放在 `setCountdown(prev => { ... })` 的回调函数内执行。React 要求状态更新器函数是纯函数（无副作用）。虽然在当前 React 版本中能工作，但这是反模式，可能在未来版本中引发问题。

**修复建议:** 将副作用移出状态更新器，使用单独的 `useEffect` 在 `countdown` 变化时触发，或使用 `useRef` 跟踪倒计时并在主 effect 中处理归零逻辑。