# 待办事项

## 待分析 / 待修复
- [ ] **分析并优化 Gemini 400 错误处理逻辑**
  - **背景**: 调用 Gemini 高级模型时，上游 API 偶发性返回 `400 {"error":{"code":400,"message":"User location is not supported for the API use.","status":"FAILED_PRECONDITION"}}` 报错。
  - **问题**: 这并非账号或参数问题，而是偶发性报错。当前系统似乎直接返回了 400 报错，导致调用方的软件中断。
  - **目标**: 分析目前的 400 错误处理逻辑，评估遇到此类 400 错误时，是**直接发起重试**更优，还是**切换账号（Tokens）并重试**更优，并给出相应的修复方案。

