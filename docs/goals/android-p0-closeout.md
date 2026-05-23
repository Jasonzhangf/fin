# Android P0 收口 Goal

## 主目标
完成 Android 手机端 P0 收口：真机闭环 + turn-channel 主链全绿 + 误报治理，形成单一可验证真相。

## 硬约束
1. 先证据后结论；无日志不宣称通过
2. 禁止 fallback / 伪造状态
3. 只修脚本不对业务语义动手
4. 不做 UI/架构扩展
5. 所有结果落盘到 `reports/`，更新 closeout 文档

## 执行阶段与 Gate

### Phase A：环境基线确认
- A1: `adb devices` + `adb -s 100.127.23.27:1234 get-state` → 设备在线证明
- A2: `cargo build -p fin-cli --manifest-path rust/Cargo.toml` → 构建成功
- A3: 启动 daemon + `cd android-client && ./gradlew :app:assembleDebug`

### Phase B：核心闭环验证
| Gate | 执行命令 | 验收标准 | 证据路径 |
|------|----------|----------|----------|
| G1 真机 live e2e | `python3 scripts/android-mvp/run_tailscale_live_e2e.py` | `tailscale-live-e2e-status.json` ok=true，含 subscribed/healthy | `reports/android-mvp-logs/tailscale-live-e2e-status.json` |
| G2 contract | `python3 scripts/android-mvp/run_turn_channel_contract.py` | 8 个字段全部 present | `reports/android-mvp-logs/turn-channel-contract.log` |
| G3 e2e | `python3 scripts/android-mvp/run_turn_channel_e2e.py` | E1/E2/E3/E4 全绿 | `reports/android-mvp-logs/turn-channel-e2e.log` |

### Phase C：误报治理
| Gate | 执行命令 | 验收标准 | 证据路径 |
|------|----------|----------|----------|
| G4 unit | `python3 scripts/android-mvp/run_turn_channel_unit.py` | ok=true（对齐 renderOneTurn 结构） | `reports/android-mvp-logs/turn-channel-unit.log` |
| G5 toggle | `python3 scripts/android-mvp/run_turn_channel_toggle_check.py` | ok=true（对齐 WS handler 分层） | `reports/android-mvp-logs/turn-channel-toggle.log` |

若 FAIL → 先确认 mobile-shell.html 业务语义正确 → 仅修检测脚本匹配逻辑 → 回归验证

### Phase D：收口文档
- 更新 `reports/android-mvp-closeout-2026-05-18.md`
- 包含：本轮执行时间、命令清单、Gate 结果、证据路径、阻塞点（若有）、P1/P2 下一步
- 最终写明：P0 PASS 或 P0 FAIL + 唯一阻塞点

## 交付物
- 所有 log 文件（reports/android-mvp-logs/）
- 更新的 closeout 报告

## 完成信号
G1-G5 全部 PASS → P0 PASS → 更新报告写明"P0 完成"
任一 G1/G2/G3 FAIL → P0 FAIL → 输出唯一阻塞点
