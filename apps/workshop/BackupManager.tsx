import React, {
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { SavePicker } from "./CartActions";
import { saveSession, saveObserver } from "./save-workspace";
import { desktop } from "./desktop-bridge";
type Backup = {
  backup_id: string;
  timestamp: string;
  action: string;
  file_count: number;
};
export function BackupManager() {
  if (!desktop)
    return (
      <section className="backup-page">
        <h2>存档备份与管理</h2>
        <p>请打开后端版查看本机备份。</p>
      </section>
    );
  return <ConnectedBackups />;
}
function ConnectedBackups() {
  const state = useSyncExternalStore(
    saveSession!.subscribe,
    saveSession!.getSnapshot,
  );
  const operation = useSyncExternalStore(
    saveObserver!.subscribe,
    saveObserver!.getSnapshot,
  );
  const [items, setItems] = useState<Backup[]>([]),
    [selected, setSelected] = useState<string[]>([]),
    [busy, setBusy] = useState(false),
    [message, setMessage] = useState(""),
    [plan, setPlan] = useState<{ id: string; backup: string } | null>(null),
    [confirmed, setConfirmed] = useState(false);
  const loadedSnapshot = useRef("");
  const locked = busy || state.busy || !saveObserver!.canStart();
  async function load() {
    if (!state.selected) return;
    setBusy(true);
    try {
      const result = await saveObserver!.run(() =>
        window.operations.execute({
          method: "save.backups",
          params: { save_id: state.selected!.save_id },
        }),
      );
      if (result && "backups" in result) {
        setItems(result.backups);
        setSelected([]);
      }
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  }
  useEffect(() => {
    if (
      state.inventory &&
      !locked &&
      loadedSnapshot.current !== state.inventory.snapshot_id
    ) {
      loadedSnapshot.current = state.inventory.snapshot_id;
      void load();
      setPlan(null);
      setConfirmed(false);
    }
  }, [state.inventory?.snapshot_id, state.busy, busy, operation.phase]);
  async function prepare() {
    setBusy(true);
    try {
      const p = await saveSession!.prepareRestore(selected[0]);
      setPlan({ id: p.plan_id, backup: selected[0] });
      setConfirmed(false);
      setMessage("恢复会替换当前角色存档，执行前会自动备份当前状态。");
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function restore() {
    if (!plan || !confirmed) return;
    setBusy(true);
    try {
      const receipt = await saveSession!.commit(plan.id);
      setPlan(null);
      setMessage(
        receipt.commit_status.startsWith("committed")
          ? "备份已恢复。"
          : "恢复结果尚未确认，请核对操作回执。",
      );
      if (receipt.commit_status.startsWith("committed"))
        await saveSession!.refresh();
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function recycle() {
    setBusy(true);
    try {
      const result = await saveObserver!.run(() =>
        window.operations.execute({
          method: "save.recycle_backups",
          params: {
            save_id: state.selected!.save_id,
            backup_ids: selected as [string, ...string[]],
          },
        }),
      );
      if (result && "backups" in result) {
        setItems(result.backups);
        setSelected([]);
        setPlan(null);
        setMessage("选中的备份已移入回收站。");
      }
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  }
  const labels: Record<string, string> = {
    "scroll-install": "添加绘卷",
    "local-effect-edit": "编辑绘卷",
    "local-scroll-delete": "删除绘卷",
    "restore-backup": "恢复前备份",
    "v2-local-edit": "编辑绘卷",
    "v2-local-delete": "删除绘卷",
    "v2-cart-install": "添加绘卷",
    "v2-live-add": "游戏内实时添加",
    "v2-count-edit": "当前可挑战次数",
  };
  return (
    <section className="backup-page" aria-busy={locked}>
      <h2>存档备份与管理</h2>
      <SavePicker />
      <div className="backup-toolbar">
        <button
          disabled={locked || !state.inventory}
          onClick={() =>
            void window.review
              .openSaveFolder({
                save_id: state.inventory!.save_id,
                snapshot_id: state.inventory!.snapshot_id,
              })
              .catch((e) => setMessage(String(e)))
          }
        >
          打开存档文件夹
        </button>
        {(["open", "set", "reset"] as const).map((action, i) => (
          <button
            key={action}
            disabled={
              locked ||
              (action !== "open" &&
                !!(state.uncertainOperationId || state.plan))
            }
            onClick={() =>
              void window.review
                .dataDirectory(action)
                .then((value) => {
                  if (value)
                    setMessage(
                      value.restart_required
                        ? "数据目录将在重启应用后生效，原目录中的备份会保留。"
                        : value.data_directory,
                    );
                })
                .catch((e) => setMessage(String(e)))
            }
          >
            {["打开数据文件夹", "更改数据目录", "恢复默认目录"][i]}
          </button>
        ))}
        <button
          disabled={locked || !state.selected}
          onClick={() => void load()}
        >
          刷新备份
        </button>
        <button
          disabled={locked}
          onClick={() =>
            void window.review
              .openBackupFolder()
              .catch((e) => setMessage(String(e)))
          }
        >
          打开备份文件夹
        </button>
        <button
          disabled={locked || selected.length !== 1}
          onClick={() => void prepare()}
        >
          恢复选中备份
        </button>
        <button
          disabled={locked || !selected.length}
          onClick={() => void recycle()}
        >
          移入回收站
        </button>
      </div>
      <div className="backup-list">
        {items.length ? (
          <table>
            <thead>
              <tr>
                <th>选择</th>
                <th>备份时间</th>
                <th>操作</th>
                <th>文件数</th>
              </tr>
            </thead>
            <tbody>
              {items.map((item) => (
                <tr key={item.backup_id}>
                  <td>
                    <input
                      type="checkbox"
                      disabled={locked}
                      aria-label={"选择备份" + item.backup_id}
                      checked={selected.includes(item.backup_id)}
                      onChange={(e) => {
                        setPlan(null);
                        setSelected(
                          e.target.checked
                            ? [...selected, item.backup_id]
                            : selected.filter((id) => id !== item.backup_id),
                        );
                      }}
                    />
                  </td>
                  <td>{item.timestamp}</td>
                  <td>{labels[item.action] || "存档备份"}</td>
                  <td>{item.file_count}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p>
            {state.selected ? "这个存档暂无备份。" : "选择存档后显示对应备份。"}
          </p>
        )}
      </div>
      {plan && (
        <div className="backup-confirm">
          <p>
            将恢复：{items.find((b) => b.backup_id === plan.backup)?.timestamp}
          </p>
          <label>
            <input
              type="checkbox"
              checked={confirmed}
              onChange={(e) => setConfirmed(e.target.checked)}
            />
            游戏已回到标题界面或已关闭
          </label>
          <button
            disabled={!confirmed || locked}
            onClick={() => void restore()}
          >
            确认恢复存档
          </button>
        </div>
      )}
      {message && <p role="status">{message}</p>}
    </section>
  );
}
