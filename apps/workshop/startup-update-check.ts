import type { UpdateState } from "../desktop/src/update-state";

export type UpdateResult = UpdateState & { canApply: boolean };
export type UpdateRequest = (params: {
  action: "status" | "check";
  channel: "stable" | "beta";
}) => Promise<UpdateResult>;

/**
 * One instance represents one application launch. It waits until packaged
 * startup cleanup is complete, requests exactly one fresh update check, and
 * asks the UI to open once for each available version it observes.
 */
export class StartupUpdateCheck {
  private checkRequested = false;
  private promptedVersion = "";
  private pending: Promise<UpdateResult> | null = null;

  constructor(
    private readonly request: UpdateRequest,
    private readonly onAvailable: () => void,
  ) {}

  poll(channel: "stable" | "beta"): Promise<UpdateResult> {
    if (this.pending) return this.pending;
    this.pending = this.pollOnce(channel).finally(() => {
      this.pending = null;
    });
    return this.pending;
  }

  private async pollOnce(channel: "stable" | "beta") {
    let state = await this.request({ action: "status", channel });
    if (
      !this.checkRequested &&
      state.canApply &&
      state.phase === "idle"
    ) {
      state = await this.request({ action: "check", channel });
      this.checkRequested = true;
    }
    if (["available", "ready"].includes(state.phase)) {
      const version = state.version || state.phase;
      if (this.promptedVersion !== version) {
        this.promptedVersion = version;
        this.onAvailable();
      }
    }
    return state;
  }
}
