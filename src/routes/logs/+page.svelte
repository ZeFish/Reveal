<script>
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { isTauri } from "@modules/core";

  /** @type {string[]} */
  let logs = $state([]);
  let filterText = $state("");
  let activeTag = $state("all");
  let autoScroll = $state(true);
  let copied = $state(false);
  /** @type {HTMLElement | null} */
  let scrollContainer = $state(null);
  /** @type {HTMLInputElement | null} */
  let searchInput = $state(null);

  const filteredLogs = $derived.by(() => {
    let result = logs;
    if (activeTag !== "all") {
      result = result.filter((l) => l.toLowerCase().includes(activeTag.toLowerCase()));
    }
    if (filterText.trim()) {
      const q = filterText.toLowerCase();
      result = result.filter((l) => l.toLowerCase().includes(q));
    }
    return result;
  });

  async function scrollToBottom() {
    if (!autoScroll || !scrollContainer) return;
    await tick();
    scrollContainer.scrollTop = scrollContainer.scrollHeight;
  }

  function handleScroll() {
    if (!scrollContainer) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollContainer;
    // Disable auto-scroll if user scrolled up more than 50px from bottom
    const atBottom = scrollHeight - scrollTop - clientHeight < 50;
    if (autoScroll !== atBottom) {
      autoScroll = atBottom;
    }
  }

  async function copyLogs() {
    const text = filteredLogs.join("\n");
    await navigator.clipboard.writeText(text);
    copied = true;
    setTimeout(() => {
      copied = false;
    }, 1500);
  }

  async function clearLogs() {
    logs = [];
    if (isTauri) {
      await invoke("clear_log_history").catch(() => {});
    }
  }

  function toggleDevtools() {
    if (isTauri) {
      invoke("toggle_devtools").catch(() => {});
    }
  }

  /** @param {string} line */
  function formatLogClass(line) {
    const lower = line.toLowerCase();
    if (lower.includes("error") || lower.includes("failed") || lower.includes("panicked")) {
      return "log-error";
    }
    if (lower.includes("[perf]") || lower.includes("ms from request")) {
      return "log-perf";
    }
    if (lower.includes("decoded in") || lower.includes("took ") || lower.includes("render ")) {
      return "log-speed";
    }
    if (lower.includes("thumb:") || lower.includes("js:")) {
      return "log-info";
    }
    return "log-default";
  }

  onMount(() => {
    if (isTauri) {
      invoke("get_log_history")
        .then((history) => {
          if (Array.isArray(history)) {
            logs = history;
            scrollToBottom();
          }
        })
        .catch(() => {});

      const unlisten = listen("log-message", (event) => {
        if (typeof event.payload === "string") {
          logs.push(event.payload);
          if (logs.length > 5000) {
            logs.shift();
          }
          if (autoScroll) {
            scrollToBottom();
          }
        }
      });

      return () => {
        unlisten.then((fn) => fn());
      };
    }
  });

  /** @param {KeyboardEvent} e */
  function handleKeydown(e) {
    if ((e.metaKey || e.ctrlKey) && e.key === "f") {
      e.preventDefault();
      searchInput?.focus();
    }
    if ((e.metaKey || e.ctrlKey) && e.key === "k") {
      e.preventDefault();
      clearLogs();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="logs-window">
  <!-- Top Toolbar -->
  <header class="toolbar">
    <div class="header-left">
      <h1 class="title">Activity Logs</h1>
      <span class="count-badge">
        {filteredLogs.length}
        {#if filteredLogs.length !== logs.length}
          / {logs.length}
        {/if}
      </span>
    </div>

    <!-- Quick Tags -->
    <div class="tags">
      {#each ["all", "[perf]", "error", "thumb", "decode"] as tag}
        <button
          type="button"
          class="tag-btn"
          class:active={activeTag === tag}
          onclick={() => { activeTag = tag; scrollToBottom(); }}
        >
          {tag}
        </button>
      {/each}
    </div>

    <!-- Search input -->
    <div class="search-box">
      <input
        bind:this={searchInput}
        type="search"
        placeholder="Filter logs (⌘F)..."
        bind:value={filterText}
      />
      {#if filterText}
        <button type="button" class="clear-search" onclick={() => (filterText = "")}>×</button>
      {/if}
    </div>

    <!-- Actions -->
    <div class="actions">
      <button
        type="button"
        class="action-btn"
        class:active={autoScroll}
        title="Toggle auto-scroll to bottom"
        onclick={() => { autoScroll = !autoScroll; if (autoScroll) scrollToBottom(); }}
      >
        Auto-scroll
      </button>
      <button type="button" class="action-btn" onclick={copyLogs} title="Copy logs to clipboard">
        {copied ? "Copied!" : "Copy"}
      </button>
      <button type="button" class="action-btn" onclick={clearLogs} title="Clear logs (⌘K)">
        Clear
      </button>
      <button type="button" class="action-btn devtools-btn" onclick={toggleDevtools} title="Open WebKit Inspector">
        Inspect
      </button>
    </div>
  </header>

  <!-- Log Content Area -->
  <main class="log-stream" bind:this={scrollContainer} onscroll={handleScroll}>
    {#if filteredLogs.length === 0}
      <div class="empty-state">
        {#if logs.length === 0}
          No logs captured yet.
        {:else}
          No logs match the current filter.
        {/if}
      </div>
    {:else}
      <div class="log-lines">
        {#each filteredLogs as line, i}
          <div class="log-row {formatLogClass(line)}">
            <span class="line-num">{i + 1}</span>
            <span class="line-content">{line}</span>
          </div>
        {/each}
      </div>
    {/if}
  </main>
</div>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    background-color: var(--color-background);
    color: var(--color-foreground);
    font-family: var(--font-monospace);
    user-select: text;
    overflow: hidden;
    height: 100vh;
  }

  .logs-window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background-color: var(--color-background);
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px;
    background-color: var(--color-surface);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .title {
    font-size: 13px;
    font-weight: 600;
    margin: 0;
    letter-spacing: 0.02em;
    color: var(--color-foreground);
  }

  .count-badge {
    font-size: 11px;
    color: var(--color-muted);
    background: var(--color-surface-raised);
    padding: 2px 6px;
    border-radius: 4px;
    font-variant-numeric: tabular-nums;
  }

  .tags {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .tag-btn {
    background: transparent;
    border: 1px solid var(--color-border);
    color: var(--color-muted);
    font-size: 11px;
    padding: 2px 7px;
    border-radius: 4px;
    cursor: pointer;
    font-family: inherit;
    transition: all var(--transition-fast);
  }

  .tag-btn:hover {
    background: var(--color-hover);
    color: var(--color-foreground);
  }

  .tag-btn.active {
    background: var(--color-surface-raised);
    color: var(--color-foreground);
    border-color: var(--color-foreground);
    font-weight: 600;
  }

  .search-box {
    position: relative;
    flex: 1;
    min-width: 140px;
    max-width: 280px;
  }

  .search-box input {
    width: 100%;
    box-sizing: border-box;
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border);
    color: var(--color-foreground);
    font-size: 12px;
    padding: 4px 24px 4px 8px;
    border-radius: 4px;
    font-family: inherit;
    outline: none;
  }

  .search-box input:focus {
    border-color: var(--color-primary);
  }

  .clear-search {
    position: absolute;
    right: 4px;
    top: 50%;
    transform: translateY(-50%);
    background: transparent;
    border: none;
    color: var(--color-muted);
    cursor: pointer;
    font-size: 14px;
    line-height: 1;
  }

  .actions {
    display: flex;
    gap: 6px;
    margin-left: auto;
  }

  .action-btn {
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    color: var(--color-muted);
    font-size: 11px;
    padding: 4px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-family: inherit;
    transition: all var(--transition-fast);
  }

  .action-btn:hover {
    background: var(--color-hover);
    color: var(--color-foreground);
  }

  .action-btn.active {
    background: var(--color-surface-raised);
    border-color: var(--color-success);
    color: var(--color-success);
  }

  .devtools-btn {
    background: var(--color-surface-raised);
    border-color: var(--color-border);
    color: var(--color-info);
  }

  .devtools-btn:hover {
    background: var(--color-hover);
    color: var(--color-info);
  }

  .log-stream {
    flex: 1;
    overflow-y: auto;
    overflow-x: auto;
    padding: 8px 0;
    font-size: 12px;
    line-height: 1.5;
  }

  .empty-state {
    padding: 40px;
    text-align: center;
    color: var(--color-muted);
    font-size: 13px;
  }

  .log-lines {
    display: flex;
    flex-direction: column;
  }

  .log-row {
    display: flex;
    align-items: baseline;
    padding: 1px 16px;
    white-space: pre-wrap;
    word-break: break-all;
    font-family: inherit;
  }

  .log-row:hover {
    background-color: var(--color-hover);
  }

  .line-num {
    display: inline-block;
    width: 44px;
    min-width: 44px;
    color: var(--color-subtle);
    font-size: 11px;
    text-align: right;
    padding-right: 14px;
    user-select: none;
    font-variant-numeric: tabular-nums;
  }

  .line-content {
    flex: 1;
  }

  /* Log highlight styles */
  .log-perf .line-content {
    color: var(--color-warning);
  }

  .log-speed .line-content {
    color: var(--color-success);
  }

  .log-error .line-content {
    color: var(--color-error);
    font-weight: 600;
  }

  .log-info .line-content {
    color: var(--color-info);
  }

  .log-default .line-content {
    color: var(--color-foreground);
  }
</style>
