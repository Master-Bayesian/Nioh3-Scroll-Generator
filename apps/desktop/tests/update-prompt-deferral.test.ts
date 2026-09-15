/**
 * The startup update prompt must open exactly once and must never stack on top
 * of another modal.
 *
 * The workshop's `openUpdatePrompt` / `handleDialogClose` pair used to hold this
 * decision inline, where no test could reach it. The decision is now the pure
 * `decideUpdatePrompt(dialogOpen, pending)` seam, and this test drives the exact
 * sequence the component performs: a prompt arriving while a dialog is open is
 * deferred, not dropped, and is re-issued once that dialog closes.
 */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { decideUpdatePrompt } from '../../workshop/use-dialog-backdrop-dismiss';

/** Mirrors the component's state machine using the shared decision function. */
function createPromptController() {
  const state = {dialogOpen: false, pending: false, opened: 0};
  return {
    openUpdatePrompt() {
      const decision = decideUpdatePrompt(state.dialogOpen, state.pending);
      if (decision.action === 'defer') {
        state.pending = true;
        return decision.action;
      }
      state.pending = false;
      state.opened += 1;
      return decision.action;
    },
    closeDialog() {
      state.dialogOpen = false;
      if (!state.pending) return null;
      const decision = decideUpdatePrompt(false, true);
      state.pending = false;
      if (decision.action === 'reopen') state.opened += 1;
      return decision.action;
    },
    state,
  };
}

test('a prompt with no dialog open opens immediately', () => {
  const prompts = createPromptController();
  assert.equal(prompts.openUpdatePrompt(), 'open');
  assert.equal(prompts.state.opened, 1);
  assert.equal(prompts.state.pending, false);
});

test('a prompt arriving while a dialog is open is deferred, then reopened once', () => {
  const prompts = createPromptController();
  // Another dialog owns the modal layer.
  prompts.state.dialogOpen = true;
  assert.equal(prompts.openUpdatePrompt(), 'defer');
  assert.equal(prompts.state.opened, 0, 'the prompt must not stack on the open dialog');
  assert.equal(prompts.state.pending, true, 'the intent must be remembered, not dropped');

  // Startup polling can ask again while the dialog is still open; the intent is
  // already recorded and nothing opens.
  assert.equal(prompts.openUpdatePrompt(), 'defer');
  assert.equal(prompts.state.opened, 0);

  // Closing the dialog issues the deferred prompt exactly once.
  assert.equal(prompts.closeDialog(), 'reopen');
  assert.equal(prompts.state.opened, 1);
  assert.equal(prompts.state.pending, false);
});

test('closing a dialog with no deferred prompt opens nothing', () => {
  const prompts = createPromptController();
  prompts.state.dialogOpen = true;
  assert.equal(prompts.closeDialog(), null);
  assert.equal(prompts.state.opened, 0);
});

test('the decision never reports reopen while the dialog is still open', () => {
  assert.deepEqual(decideUpdatePrompt(true, false), {action: 'defer'});
  assert.deepEqual(decideUpdatePrompt(true, true), {action: 'defer'});
  assert.deepEqual(decideUpdatePrompt(false, false), {action: 'open'});
  assert.deepEqual(decideUpdatePrompt(false, true), {action: 'reopen'});
});
