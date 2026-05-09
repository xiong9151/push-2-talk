/**
 * ResultPanelWindow 键盘快捷键动作判断
 *
 * 提取为纯函数以便测试
 */

/** 结果面板支持的动作 */
export type ResultPanelAction = "dismiss";

/**
 * 根据按键事件返回应执行的动作
 *
 * - Escape → dismiss（关闭面板）
 * - Ctrl+C / Meta+C → null（不拦截，保留 WebView 原生文本选择复制）
 * - 其他按键 → null
 */
export function getKeyboardAction(
  key: string,
  ctrlKey: boolean,
  metaKey: boolean,
): ResultPanelAction | null {
  // 不拦截 Ctrl+C / Cmd+C（保留原生复制行为）
  if ((ctrlKey || metaKey) && key.toLowerCase() === "c") {
    return null;
  }

  if (key === "Escape") {
    return "dismiss";
  }

  return null;
}
