<script>
  // A quiet link to one page of the manual, for the places where a control
  // needs a sentence the interface has no room for.
  import Icon from "./Icon.svelte";
  import { openManual } from "../manual.js";

  /** @type {{ page: string, label: string }} */
  let { page, label } = $props();
</script>

<button type="button" class="ghost small" title="Open in the manual" onclick={() => openManual(page)}>
  <Icon name="book-open" size="var(--icon-md)" />
  <span>{label}</span>
</button>
