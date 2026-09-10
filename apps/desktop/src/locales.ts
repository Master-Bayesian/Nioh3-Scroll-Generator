/** Presentation resources only. IDs, numeric requests and job state stay language-neutral. */
export const locales = ['zh-CN', 'en-US', 'ja-JP'] as const;
export type Locale = typeof locales[number];
export function normalizeLocale(value: string): Locale {
  const language = value.replaceAll('_', '-').split('-')[0].toLowerCase();
  return language === 'zh' ? 'zh-CN' : language === 'ja' ? 'ja-JP' : 'en-US';
}
export function isLocale(value: unknown): value is Locale { return locales.includes(value as Locale); }
export const english = {
  exportDiagnostics: 'Export diagnostics', diagnosticsSaved: 'Diagnostics saved.', diagnosticsCancelled: 'Export cancelled.',
  title: 'Nioh 3 — V2 Engineering Workbench', intro: 'Search integration reference. Final navigation, layout and visual design await Figma.',
  language: 'Language', worker: 'Worker', ready: 'Ready — offline search', connecting: 'Connecting…', disconnected: 'Disconnected',
  restart: 'Restart offline worker', capabilities: 'Context and capabilities', queryTitle: 'Query draft',
  queryHint: 'IDs define constraints. Enemy requirements mean “must contain”. Editing this draft does not change a submitted job.',
  queryJson: 'Search query JSON', results: 'Results', budget: 'Job trial budget', cpu: 'Allow bulk CPU fallback (may be slow)',
  start: 'Start search', cancel: 'Cancel search', resume: 'Resume submitted query', candidates: 'Job and candidates',
  candidateCount: '{count} candidates', cursor: 'Committed cursor: {cursor} · Elapsed: {time} ms',
  cancelPending: 'Cancellation accepted; waiting for the current offline call to stop.', progress: 'Latest page progress (not a global percentage)',
  seed: 'Seed', rarity: 'Rarity', stage: 'Stage', effects: 'Effects (name · ID : value)',
  evidence: 'Evidence: certified offline replay. Broker installation uses candidate IDs and a prepared save plan.',
  operations: 'Save and runtime operations',
  saveHint: 'Save changes require a snapshot, a prepared plan, then an explicit save.commit command. Keep the plan ID to query save.operation after a connection error. Never replay an uncertain write automatically.',
  nativeHint: 'Native generation requires the title screen. Live in-game insertion remains a research item.',
  selectSave: 'Select character save', command: 'Operation command JSON', execute: 'Execute operation command',
  recoverSave: 'Recover save operation', recoverRuntime: 'Recover runtime operation', cancelOperation: 'Cancel operation',
  inspectReceipt: 'Inspect operation receipt', operationUnknown: 'The connection was interrupted. The operation may still be running. Recover its status before starting another operation.',
  diagnostics: 'Technical details', invalidInput: 'Check the input values and try again.',
  unavailable: 'The worker connection is unavailable. Check its status before continuing.',
  operationFailed: 'The operation could not complete. Inspect its receipt before retrying a write.',
  contextChanged: 'The generation context has changed. Refresh the worker and submit a new request.',
  busy: 'An operation is already running. Wait for its result.', preferencesFailed: 'The language preference could not be saved.',
  catalogFailed: 'Localized names could not be loaded. Numeric IDs remain available.',
  queued: 'Queued', running: 'Running', cancel_requested: 'Cancellation requested', completed: 'Completed', cancelled: 'Cancelled', failed: 'Failed',
  result_limit: 'Requested results found', budget_reached: 'Trial budget reached', family_exhausted: 'Search family exhausted', error: 'Error', working: 'Working',
  final_record: 'Final record', native_stage_one: 'Pending finalization', effect_sequence_only: 'Effect sequence preview',
  shutdownTitle: 'Operation still owned by worker', shutdownMessage: 'A safe shutdown could not be confirmed. Wait for the operation or stop the runtime override, then close again. No worker was force-terminated.',
} as const;
export type MessageKey = keyof typeof english;
export type Messages = Record<MessageKey, string>;
const chinese: Messages = {
  exportDiagnostics: '导出诊断信息', diagnosticsSaved: '诊断信息已保存。', diagnosticsCancelled: '已取消导出。',
  title: '仁王3 — V2 工程工作台', intro: '搜索集成验证界面。最终导航、布局和视觉设计等待 Figma。',
  language: '语言', worker: '工作进程', ready: '就绪 — 离线搜索', connecting: '连接中…', disconnected: '未连接',
  restart: '重启离线工作进程', capabilities: '生成上下文与能力', queryTitle: '搜索条件草稿',
  queryHint: '约束以 ID 为准。敌人条件表示“必须包含”。修改草稿不会改变已提交的任务。',
  queryJson: '搜索条件 JSON', results: '结果数量', budget: '任务尝试次数上限', cpu: '允许批量 CPU 回退（可能较慢）',
  start: '开始搜索', cancel: '取消搜索', resume: '继续已提交的搜索', candidates: '任务与候选绘卷',
  candidateCount: '{count} 个候选', cursor: '已确认游标：{cursor} · 用时：{time} 毫秒',
  cancelPending: '已接受取消请求，正在等待当前离线调用结束。', progress: '最近一页的进度（不是全局百分比）',
  seed: 'Seed', rarity: '稀有度', stage: '阶段', effects: '词条（名称 · ID：数值）',
  evidence: '证据：已验证的离线重放。安装通过候选 ID 和预先生成的存档操作计划完成。',
  operations: '存档与运行时操作',
  saveHint: '存档修改需要先读取快照、生成计划，再明确执行 save.commit。连接异常后保留计划 ID，并用 save.operation 查询结果。不要自动重试结果不明的写入。',
  nativeHint: '原生生成要求停留在标题界面。游戏内实时添加仍属于研究项目。',
  selectSave: '选择角色存档', command: '操作命令 JSON', execute: '执行操作命令',
  recoverSave: '恢复存档操作状态', recoverRuntime: '恢复运行时操作状态', cancelOperation: '取消操作',
  inspectReceipt: '查询操作回执', operationUnknown: '连接已中断，操作可能仍在进行。请先恢复状态，再开始新的操作。',
  diagnostics: '技术详情', invalidInput: '请检查输入内容后重试。',
  unavailable: '工作进程连接不可用，请先检查状态。', operationFailed: '操作未能完成。再次写入前请检查操作回执。',
  contextChanged: '生成上下文已变化，请刷新工作进程并重新提交。', busy: '已有操作正在运行，请等待结果。',
  preferencesFailed: '无法保存语言偏好。', catalogFailed: '无法加载本地化名称，仍可查看数值 ID。',
  queued: '排队中', running: '运行中', cancel_requested: '已请求取消', completed: '已完成', cancelled: '已取消', failed: '失败',
  result_limit: '已找到所需结果', budget_reached: '已达尝试次数上限', family_exhausted: '当前搜索族已穷尽', error: '错误', working: '处理中',
  final_record: '最终记录', native_stage_one: '等待最终解析', effect_sequence_only: '词条序列预览',
  shutdownTitle: '工作进程仍持有操作', shutdownMessage: '尚无法确认安全退出。请等待操作结束或停止临时运行时覆盖后再次关闭。工作进程未被强制结束。',
};
const japanese: Messages = {
  exportDiagnostics: '診断情報をエクスポート', diagnosticsSaved: '診断情報を保存しました。', diagnosticsCancelled: 'エクスポートをキャンセルしました。',
  title: '仁王3 — V2 開発ワークベンチ', intro: '検索機能の結合確認用画面です。最終的な構成とデザインは Figma で決定します。',
  language: '言語', worker: 'ワーカープロセス', ready: '準備完了 — オフライン検索', connecting: '接続中…', disconnected: '未接続',
  restart: 'オフラインワーカーを再起動', capabilities: '生成コンテキストと機能', queryTitle: '検索条件の下書き',
  queryHint: '条件は ID で指定します。敵の条件は「必ず含む」を意味します。下書きを変更しても送信済みのジョブは変わりません。',
  queryJson: '検索条件 JSON', results: '結果数', budget: '試行回数の上限', cpu: 'CPU への一括処理の切り替えを許可（時間がかかる場合があります）',
  start: '検索開始', cancel: '検索をキャンセル', resume: '送信済みの検索を再開', candidates: 'ジョブと絵巻候補',
  candidateCount: '候補 {count} 件', cursor: '確定済みカーソル：{cursor} · 経過時間：{time} ミリ秒',
  cancelPending: 'キャンセルを受け付けました。現在のオフライン処理の終了を待っています。', progress: '直近のページの進捗（全体の割合ではありません）',
  seed: 'Seed', rarity: '希少度', stage: '段階', effects: '特殊効果（名前 · ID：値）',
  evidence: '根拠：検証済みのオフライン再現。追加は候補 ID と事前に作成したセーブ操作計画を使用します。',
  operations: 'セーブとランタイムの操作',
  saveHint: 'セーブの変更には、スナップショットの取得、計画の作成、明示的な save.commit の実行が必要です。接続エラー時は計画 ID を保持し、save.operation で結果を確認してください。結果不明の書き込みを自動で再試行しないでください。',
  nativeHint: 'ネイティブ生成はタイトル画面で実行します。ゲーム内での直接追加は研究段階です。',
  selectSave: 'キャラクターのセーブを選択', command: '操作コマンド JSON', execute: '操作コマンドを実行',
  recoverSave: 'セーブ操作の状態を復元', recoverRuntime: 'ランタイム操作の状態を復元', cancelOperation: '操作をキャンセル',
  inspectReceipt: '操作結果を照会', operationUnknown: '接続が中断されました。操作が続いている可能性があります。次の操作の前に状態を確認してください。',
  diagnostics: '技術的な詳細', invalidInput: '入力内容を確認して、もう一度お試しください。',
  unavailable: 'ワーカーに接続できません。続行する前に状態を確認してください。', operationFailed: '操作を完了できませんでした。書き込みを再試行する前に操作結果を確認してください。',
  contextChanged: '生成コンテキストが変わりました。ワーカーを更新して再送信してください。', busy: '別の操作を実行中です。結果をお待ちください。',
  preferencesFailed: '言語設定を保存できませんでした。', catalogFailed: '翻訳された名前を読み込めませんでした。数値 ID は引き続き確認できます。',
  queued: '待機中', running: '実行中', cancel_requested: 'キャンセル要求済み', completed: '完了', cancelled: 'キャンセル済み', failed: '失敗',
  result_limit: '必要な結果数に到達', budget_reached: '試行回数の上限に到達', family_exhausted: '現在の検索群を走査済み', error: 'エラー', working: '処理中',
  final_record: '最終レコード', native_stage_one: '最終確定待ち', effect_sequence_only: '特殊効果のプレビュー',
  shutdownTitle: 'ワーカーが操作を保持しています', shutdownMessage: '安全に終了できることを確認できません。処理の終了を待つか、一時的なランタイム変更を停止してから再度閉じてください。ワーカーは強制終了されていません。',
};
export const messages: Record<Locale, Messages> = { 'en-US': english, 'zh-CN': chinese, 'ja-JP': japanese };
export function translate(locale: Locale, key: MessageKey, values: Record<string, string | number> = {}): string {
  return messages[locale][key].replace(/\{(\w+)\}/g, (_match, name: string) => {
    if (!(name in values)) throw new Error(`MISSING_MESSAGE_ARGUMENT: ${key}.${name}`);
    return typeof values[name] === 'number' ? new Intl.NumberFormat(locale).format(values[name]) : values[name];
  });
}
export function localizedError(locale: Locale, error: unknown): string {
  const text = String(error);
  const key: MessageKey = /CONTEXT_MISMATCH|CONTRACT_MISMATCH/.test(text) ? 'contextChanged'
    : /INVALID_REQUEST|SyntaxError|INVALID_INPUT/.test(text) ? 'invalidInput'
    : /\bBUSY\b/.test(text) ? 'busy'
    : /WORKER_UNAVAILABLE|WORKER_EXITED|ENOENT|WORKER_TIMEOUT/.test(text) ? 'unavailable' : 'operationFailed';
  return translate(locale, key);
}
