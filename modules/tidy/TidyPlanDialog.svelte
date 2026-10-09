<script>
  // "Tidy folder": what filing this folder by the import rule would do.
  //
  // Shows the filing plan and allows applying it directly to file photos
  // into day folders according to preferences.
  import { untrack } from "svelte";
  import Dialog from "@stnd/ui/Dialog.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { isTauri, notify } from "@modules/core";
  import { ManualLink } from "@modules/core";
  import { applyTidyPlan } from "./tidyOperations.js";

  /**
   * `preview` is a plan that is already made: tests and screenshots hand one
   * in; the app never does, and the dialog asks for its own.
   * @type {{ dir: string, onClose?: () => void, preview?: any, onApplied?: (result: any) => void }}
   */
  let { dir, onClose = () => {}, preview = null, onApplied = () => {} } = $props();

  /** @type {any} */
  let plan = $state(untrack(() => preview));
  let error = $state("");
  let progress = $state({ done: 0, total: 0 });

  let applying = $state(false);
  // Moving is on the NAS and real: the first click asks, the second moves.
  let confirming = $state(false);
  let applyProgress = $state({ done: 0, total: 0, current: "" });
  /** @type {{ moved: number, errors: string[] } | null} */
  let appliedResult = $state(null);

  const name = (/** @type {string} */ p) => p.split("/").filter(Boolean).pop() ?? p;
  const n = (/** @type {number} */ v) => v.toLocaleString("en-CA");
  /** A path shown relative to the archive it files into. */
  const under = (/** @type {string} */ p) => {
    const base = plan?.base ?? "";
    return p.startsWith(`${base}/`) ? p.slice(base.length + 1) : p;
  };

  /** @param {{ reasons: [string, number][] }} g */
  const reasonsText = (g) => g.reasons.map(([r, c]) => `${n(c)} ${r}`).join(" · ");
  /** @param {{ from_dirs: [string, number][] }} g */
  const fromText = (g) => g.from_dirs.map(([f, c]) => `${name(f)} (${n(c)})`).join(", ");

  $effect(() => {
    if (!isTauri || preview) return;
    let off = () => {};
    listen("tidy-progress", (e) => {
      const p = /** @type {any} */ (e.payload);
      if (typeof p.done === "number") progress = { done: p.done, total: p.total };
    }).then((u) => (off = u));
    invoke("tidy_plan", { dir })
      .then((result) => (plan = result))
      .catch((e) => (error = String(e)));
    return () => off();
  });

  $effect(() => {
    if (!isTauri) return;
    let off = () => {};
    listen("tidy-apply-progress", (e) => {
      const p = /** @type {any} */ (e.payload);
      if (typeof p.done === "number") {
        applyProgress = { done: p.done, total: p.total || 0, current: p.current || "" };
      }
    }).then((u) => (off = u));
    return () => off();
  });

  async function handleApply() {
    if (applying || !isTauri || !dir) return;
    confirming = false;
    applying = true;
    error = "";
    try {
      const res = await applyTidyPlan(dir, { invoke });
      appliedResult = res;
      notify(`${res.moved} photo${res.moved === 1 ? "" : "s"} filed by import rule ✓`);
      onApplied(res);
    } catch (e) {
      error = String(e);
    } finally {
      applying = false;
    }
  }
</script>

<Dialog open label="Tidy folder" onclose={onClose} style="--dialog-width: min(92vw, 820px); --dialog-max-height: 88vh">
  <header class="head">
    <h2>Tidy “{name(dir)}”</h2>
    {#if appliedResult}
      <p class="fine">Filing complete.</p>
    {:else}
      <p class="fine">Review the filing plan below and apply when ready.</p>
    {/if}
    <ManualLink page="reference/files/#tidy-folder" label="How Tidy plans and protects your files" />
  </header>

  <div class="body">
  {#if error}
    <div role="alert" class="alert error">{error}</div>
  {:else if appliedResult}
    <div class="result-box">
      <h3>Filing complete ✓</h3>
      <p><b>{n(appliedResult.moved)}</b> photo{appliedResult.moved === 1 ? "" : "s"} moved to day folders.</p>
      {#if appliedResult.errors?.length}
        <details class="alert error">
          <summary>{appliedResult.errors.length} issue(s) encountered</summary>
          <ul>
            {#each appliedResult.errors as err}
              <li>{err}</li>
            {/each}
          </ul>
        </details>
      {/if}
    </div>
  {:else if applying}
    <div class="applying-box">
      <p class="muted">
        Moving photos into day folders…
        {#if applyProgress.total}{n(applyProgress.done)} of {n(applyProgress.total)}{/if}
        {#if applyProgress.current}
          <span class="fine">({applyProgress.current})</span>
        {/if}
      </p>
      <progress max={applyProgress.total || 1} value={applyProgress.done}></progress>
    </div>
  {:else if !plan}
    <p class="muted">
      Reading photo dates…
      {#if progress.total}{n(progress.done)} of {n(progress.total)}{/if}
    </p>
    <progress max={progress.total || 1} value={progress.done}></progress>
  {:else}
    {@const s = plan.summary}
    <section class="rule">
      <p>
        The rule: <code>{plan.pattern}</code>, filed under <code title={plan.base}>…/{plan.base.split("/").slice(-2).join("/")}</code>
      </p>
    </section>

    <ul class="summary">
      <li><b>{n(s.photos)}</b> photos checked</li>
      <li><b>{n(s.in_place)}</b> already where the rule puts them</li>
      <li class="lead">
        <b>{n(s.to_move)}</b> would move, into {n(s.new_folders)} new day folders
        {#if s.companions}
          <span class="fine">({n(s.companions)} companion sidecars, videos and renders would follow)</span>
        {/if}
      </li>
      <li><b>{n(s.kept)}</b> kept, because they sit in folders you named</li>
      {#if s.conflicts}<li><b>{n(s.conflicts)}</b> would clash with a file already there, and would stay</li>{/if}
      {#if s.undated}
        <li>
          <b>{n(s.undated)}</b> left alone, with no usable date
          {#if s.future_dated || s.file_time_only}
            <span class="fine">
              ({#if s.future_dated}{n(s.future_dated)} dated in the future, a camera clock that was wrong{/if}{#if s.future_dated && s.file_time_only}; {/if}{#if s.file_time_only}{n(s.file_time_only)} with only a file time{/if})
            </span>
          {/if}
        </li>
      {/if}
      {#if s.other_files}<li class="muted"><b>{n(s.other_files)}</b> other files, untouched</li>{/if}
    </ul>

    {#if plan.groups.length}
      <h3>Where they would go</h3>
      <div class="list divided groups">
        {#each plan.groups as g (g.to_dir)}
          <div class="item group">
            <div class="where">
              <span class="to">{under(g.to_dir)}</span>
              <span class="fine">
                {reasonsText(g)}{#if g.from_dirs.length}, from {fromText(g)}{/if}
              </span>
            </div>
            <span class="badge">{n(g.count)}</span>
          </div>
        {/each}
      </div>
      {#if plan.more_groups}<p class="fine">…and {n(plan.more_groups)} more day folders.</p>{/if}
    {/if}

    {#if plan.kept.length}
      <h3>Kept where you put them</h3>
      <div class="list divided">
        {#each plan.kept.slice(0, 30) as [folder, count] (folder)}
          <div class="item"><span class="to">{folder}</span><span class="badge">{n(count)}</span></div>
        {/each}
      </div>
      {#if plan.kept.length > 30}<p class="fine">…and {n(plan.kept.length - 30)} more named folders.</p>{/if}
    {/if}

    {#if plan.conflicts.length}
      <details>
        <summary>{n(s.conflicts)} that would clash</summary>
        <div class="list divided">
          {#each plan.conflicts as c (c.from)}
            <div class="item clash"><span class="to">{name(c.from)}</span><span class="fine">{c.why}</span></div>
          {/each}
        </div>
        {#if plan.more_conflicts}<p class="fine">…and {n(plan.more_conflicts)} more.</p>{/if}
      </details>
    {/if}
  {/if}
  </div>

  <footer class="foot">
    <button type="button" onclick={onClose} disabled={applying}>Close</button>
    {#if !appliedResult && plan?.summary?.to_move > 0}
      {#if confirming && !applying}
        <span class="fine confirm-note">
          Moves {n(plan.summary.to_move)} photo{plan.summary.to_move === 1 ? "" : "s"} and their sidecars on disk. Nothing is overwritten.
        </span>
        <button type="button" onclick={() => (confirming = false)}>Back</button>
        <button type="button" class="apply-btn" onclick={handleApply}>
          Move {n(plan.summary.to_move)}
        </button>
      {:else}
        <button
          type="button"
          class="apply-btn"
          onclick={() => (confirming = true)}
          disabled={applying || !isTauri}
        >
          {#if applying}
            Moving…
          {:else}
            Apply ({n(plan.summary.to_move)} to move)
          {/if}
        </button>
      {/if}
    {/if}
  </footer>
</Dialog>

<style>
  .head, .body, .foot { padding-inline: var(--space); }
  .head { padding-block-start: var(--space); }
  .head h2 { margin: 0 0 var(--space-d4); }
  .body { display: flex; flex-direction: column; gap: var(--space-d2); padding-block: var(--space-d2); }
  .body > :global(*) { margin: 0; }
  .summary { padding: 0; list-style: none; display: grid; gap: var(--space-d4); }
  /* The framework draws a bullet on every list item, even in a list that has none; this summary
     is a column of facts, not bullets. */
  .summary li::before { content: none; }
  .summary .lead { font-size: 1.1em; }
  h3 { margin-block-start: var(--space-d2); }
  .groups { max-height: 18rem; overflow-y: auto; }
  .group { justify-content: space-between; }
  .where { display: flex; flex-direction: column; min-width: 0; }
  .to { font-family: var(--font-monospace); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .clash { justify-content: space-between; gap: var(--space-d2); }
  .foot { display: flex; justify-content: flex-end; align-items: center; gap: var(--space-d2); padding-block: var(--space); }
  .confirm-note { margin-right: auto; }
  .apply-btn {
    background: var(--color-accent);
    color: var(--color-on-accent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-accent) 60%, black);
    border-radius: var(--radius-sm);
    padding: var(--space-d4) var(--space-d2);
    font-weight: 600;
    cursor: pointer;
  }
  .apply-btn:hover:not(:disabled) {
    filter: brightness(1.1);
  }
  .apply-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .result-box, .applying-box {
    display: flex;
    flex-direction: column;
    gap: var(--space-d2);
    padding-block: var(--space-d2);
  }
</style>
