<script lang="ts">
  import { ContextMenu as ContextMenuPrimitive } from "bits-ui";

  let {
    ref = $bindable(null),
    child,
    ...restProps
  }: ContextMenuPrimitive.TriggerProps = $props();
</script>

<ContextMenuPrimitive.Trigger
  bind:ref
  {child}
  data-slot="context-menu-trigger"
  {...restProps}
/>
