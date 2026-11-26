<!--
  The temporary switch of the Properties panel (docs/react-web-plan.md, section 7, slice S6): what Editor.svelte imports in place
  of PropertiesPanel.svelte. Without `/?react=properties` it draws the Svelte panel (with the bind picker that `bindParts` in
  shell.ts gives it); with it, the React panel in an island, loaded on demand so that the editor's bundle does not carry React.
  If the React chunk does not load, the Svelte panel (which is in the entry) is drawn, so that the person never has a page with
  no properties. The host element draws nothing of its own (`display: contents`). It goes with the switch when the last slice
  removes the Svelte shell.

  Two things in the Fill section are the host's, not the panel's. The image picker is Svelte until S7, so the panel draws a
  ForeignSlot and this file gives it `imageView`: a function that mounts the Svelte picker (in ImagePickerHost.svelte, which gives
  it a tooltip provider) on the slot's element and passes the next props to it. And the route of an asset (`assetUrl`) is in the
  host package web-assets, which a view package may not import, so `thumbnail` is given from here too; an id that is not an
  asset id has none.
-->
<script lang="ts">
  import { mount, unmount, type ComponentProps } from 'svelte';
  import { assetUrl, isAssetId } from '@meldframe/web-assets/assets';
  import PropertiesPanel from '@meldframe/web-panels/PropertiesPanel.svelte';
  import type { Foreign } from '@meldframe/web-panels/react/ForeignSlot';
  import type { ImagePickerProps } from '@meldframe/web-panels/react/FillSection';
  import ImagePickerHost from './ImagePickerHost.svelte';
  import { reactFor } from './pilot';

  let props: ComponentProps<typeof PropertiesPanel> = $props();
  // The React side, island helper included, loads on demand: the editor's bundle does not carry react-dom.
  const react = reactFor('properties', location.search)
    ? Promise.all([import('@meldframe/web-chrome/react/island'), import('@meldframe/web-panels/react/PropertiesPanel')]).then(([i, m]) => ({ island: i.island, component: m.PropertiesPanel }))
    : null;

  // One object for the life of the page: the slot mounts again when `view` is another one.
  const imageView: Foreign<ImagePickerProps> = {
    mount(target, first) {
      let now = $state.raw(first);
      const app = mount(ImagePickerHost, {
        target,
        props: {
          get properties() {
            return now.properties;
          },
          get assets() {
            return now.assets;
          },
          get dispatch() {
            return now.dispatch;
          },
        },
      });
      return {
        update: (next) => (now = next),
        destroy: () => void unmount(app),
      };
    },
  };
  const thumbnail = (id: string) => (isAssetId(id) ? assetUrl(id) : null);
</script>

{#if react}
  {#await react then r}
    <div
      class="contents"
      use:r.island={{
        component: r.component,
        props: { properties: props.properties, picker: props.picker, assets: props.assets, selected: props.selected, dispatch: props.dispatch, imageView, thumbnail },
      }}
    ></div>
  {:catch}
    <!-- the React chunk did not load: the Svelte panel is in the entry, so the properties are still there -->
    <PropertiesPanel {...props} />
  {/await}
{:else}
  <PropertiesPanel {...props} />
{/if}
