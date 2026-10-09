<script>
  import NotificationStack from "./NotificationStack.svelte";
  import Toast from "./Toast.svelte";
  import TaskIndicator from "./TaskIndicator.svelte";
  import RenderQueueModal from "./RenderQueueModal.svelte";
  import ShortcutsModal from "./ShortcutsModal.svelte";
  import { UpdateCard, WhatsNewModal } from "@modules/updates";
  import TidyPlanDialog from "../tidy/TidyPlanDialog.svelte";
  import CatalogueNoteModal from "./CatalogueNoteModal.svelte";
  import AdjustDateModal from "./AdjustDateModal.svelte";
  import { activity, setQueueOpen, notify, hold } from "@modules/core";
  import { cancelExportQueue as opCancelExportQueue } from "@modules/export";
  import { tidyState } from "@modules/tidy";
  import { library, patchFrames } from "@modules/library";
  import { invoke } from "@tauri-apps/api/core";
  import { modalState } from "./modalState.svelte.js";
  import { saveCatalogNote as opSaveCatalogNote } from "./modalOperations.js";

  /**
   * @typedef {Object} Props
   * @property {(() => void) | undefined} [onCancelQueue]
   * @property {string | null} [tidyDir]
   * @property {() => void} [onCloseTidy]
   * @property {boolean} [shortcutsOpen]
   * @property {() => void} [onCloseShortcuts]
   * @property {{ version: string, notes: string } | null} [whatsNew]
   * @property {() => void} [onCloseWhatsNew]
   * @property {boolean} [catalogOpen]
   * @property {string} [catalogContent]
   * @property {() => void | Promise<void>} [onSaveCatalog]
   * @property {() => void} [onCloseCatalog]
   */

  /** @type {Props} */
  let {
    onCancelQueue,
    tidyDir,
    onCloseTidy,
    shortcutsOpen,
    onCloseShortcuts,
    whatsNew,
    onCloseWhatsNew,
    catalogOpen,
    catalogContent,
    onSaveCatalog,
    onCloseCatalog,
  } = $props();

  const isShortcutsVisible = $derived(shortcutsOpen !== undefined ? shortcutsOpen : modalState.shortcutsOpen);
  const handleCloseShortcuts = () => {
    if (onCloseShortcuts) onCloseShortcuts();
    modalState.closeShortcuts();
  };

  const currentWhatsNew = $derived(whatsNew !== undefined ? whatsNew : modalState.whatsNew);
  const handleCloseWhatsNew = () => {
    if (onCloseWhatsNew) onCloseWhatsNew();
    modalState.closeWhatsNew();
  };

  const isCatalogVisible = $derived(catalogOpen !== undefined ? catalogOpen : modalState.catalogOpen);
  const currentCatalogContent = $derived(catalogContent !== undefined ? catalogContent : modalState.catalogContent);
  const handleCloseCatalog = () => {
    if (onCloseCatalog) onCloseCatalog();
    modalState.closeCatalog();
  };
  const handleSaveCatalog = async () => {
    if (onSaveCatalog) {
      await onSaveCatalog();
    } else if (library.root) {
      await opSaveCatalogNote(library.root, modalState.catalogContent, { invoke });
    }
  };

  const currentTidyDir = $derived(tidyDir !== undefined ? tidyDir : tidyState.dir);
  const handleCloseTidy = () => {
    if (onCloseTidy) onCloseTidy();
    tidyState.close();
  };

  const handleCancelQueue = () => {
    if (onCancelQueue) onCancelQueue();
    else opCancelExportQueue();
  };
</script>

<!-- The single place every long-running operation reports to, regardless of
     which mode (Grid/Develop) is currently showing. -->
<NotificationStack>
  <UpdateCard />
  <Toast message={activity.message} />
  <TaskIndicator activityQueue={activity.queue} onOpen={() => setQueueOpen(true)} />
</NotificationStack>

{#if activity.queueOpen}
  <RenderQueueModal
    activityQueue={activity.queue}
    activeActivityId={activity.activeId}
    onClose={() => setQueueOpen(false)}
    onCancelQueue={handleCancelQueue}
  />
{/if}

{#if currentTidyDir}
  <TidyPlanDialog dir={currentTidyDir} onClose={handleCloseTidy} />
{/if}

{#if isShortcutsVisible}
  <ShortcutsModal onClose={handleCloseShortcuts} />
{/if}

{#if currentWhatsNew}
  <WhatsNewModal version={currentWhatsNew.version} notes={currentWhatsNew.notes} onClose={handleCloseWhatsNew} />
{/if}

<CatalogueNoteModal
  open={isCatalogVisible}
  content={currentCatalogContent}
  onSave={handleSaveCatalog}
  onClose={handleCloseCatalog}
/>

<AdjustDateModal
  open={modalState.adjustDateOpen}
  frames={modalState.adjustDateFrames}
  onClose={() => modalState.closeAdjustDate()}
  onAdjust={async (updates) => {
    try {
      await invoke("adjust_capture_date", { updates });
      for (const u of updates) {
        patchFrames(u.path, { capture_at: u.capture_at });
      }
      notify(`Adjusted date for ${updates.length} photo${updates.length > 1 ? "s" : ""}`, 3000);
    } catch (err) {
      hold(`Failed to adjust date: ${err}`);
    }
  }}
/>
