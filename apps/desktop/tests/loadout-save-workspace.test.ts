import assert from "node:assert/strict";
import test from "node:test";
import { createLoadoutSaveGateway } from "../../workshop/loadout-save-workspace";
import type { PublicOperation } from "../src/operations-api";
import type { ProtectedJob, SaveCharacter, SaveReference } from "../../../packages/contracts/protected-responses";
import { OperationController } from "../src/operation-controller";

const reference: SaveReference = { save_id: "loadout-test", save_slot: 0, account_id: "test", path: "D:/test/SAVEDATA00" };
const character: SaveCharacter = { save_id: reference.save_id, source_sha256: "test-digest", equipment_slots: 0, equipment: [], currencies: { amrita: 0, gold: 0 } };
let serial = 0;
const completed = (kind: ProtectedJob["kind"], result: ProtectedJob["result"]): ProtectedJob => ({ job_id: String(++serial), kind, result, state: "completed", sequence: 1, cancellable: false, progress: null, error: null });
const idleApi = { current: async () => ({ job: null, busy: false }), snapshot: async () => { throw new Error("Unexpected job poll"); }, cancel: async () => { throw new Error("Unexpected cancellation"); } };

test("planner gateway sends only read operations through the existing observer", async () => {
  const operations: PublicOperation[] = [];
  let observed = 0;
  const api = {
    execute: async (command: PublicOperation) => {
      operations.push(command);
      if (command.method === "save.discover") return completed(command.method, { saves: [reference] });
      if (command.method === "save.character") return completed(command.method, character);
      throw new Error("Unexpected write or operation");
    },
    selectSave: async () => completed("save.register", reference),
  };
  const observer = new OperationController("save", { ...idleApi, ...api });
  const gateway = createLoadoutSaveGateway({ run: async action => { observed++; return observer.run(action); } }, api);
  assert.deepEqual(await gateway.discover(), [reference]);
  assert.deepEqual(await gateway.pick(), reference);
  assert.deepEqual(await gateway.read(reference), character);
  assert.equal(observed, 3);
  assert.deepEqual(operations, [{ method: "save.discover", params: {} }, { method: "save.character", params: { save_id: reference.save_id } }]);
  observer.dispose();
});

test("planner gateway preserves picker cancellation and reports backend failure", async () => {
  const api = {
    execute: async () => { throw new Error("SAVE_CHANGED: reload the save"); },
    selectSave: async () => null,
  };
  const observer = new OperationController("save", { ...idleApi, ...api });
  const gateway = createLoadoutSaveGateway(observer, api);
  assert.equal(await gateway.pick(), null);
  await assert.rejects(gateway.read(reference), /SAVE_CHANGED/);
  observer.dispose();
});

test("planner gateway rejects unrelated responses instead of inventing equipment", async () => {
  const api = { execute: async () => completed("save.discover", { saves: [] }), selectSave: async () => completed("save.register", { saves: [] }) };
  const observer = new OperationController("save", { ...idleApi, ...api });
  const gateway = createLoadoutSaveGateway(observer, api);
  await assert.rejects(gateway.read(reference));
  await assert.rejects(gateway.pick());
  observer.dispose();
});
