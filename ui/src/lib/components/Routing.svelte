<script lang="ts">
  import { sendIntent } from '../backend';
  import {
    ROUTING_OUTPUTS,
    canRoute,
    type RouteView,
    type RoutingOutputId,
    type Snapshot,
  } from '../device';
  import { i18n } from '../i18n/index.svelte';

  let { device }: { device: Snapshot | null } = $props();

  let rows = $derived(Array.isArray(device?.routing) ? device.routing : null);

  // The microphone reaches the headphones through the volume of its monitor.
  let monitorDown = $derived.by(() => {
    const routed = rows?.find((row) => row.input === 'mic')?.outputs.includes('headphones');
    const monitor = device?.channels.find((view) => view.channel === 'micMonitor');
    return routed === true && monitor !== undefined && !monitor.volume;
  });

  function label(row: RouteView, output: RoutingOutputId): string {
    return i18n.t.routing.route
      .replace('{input}', i18n.t.routing.inputs[row.input])
      .replace('{output}', i18n.t.routing.outputs[output]);
  }
</script>

<section>
  <h1>{i18n.t.nav.routing}</h1>
  {#if rows}
    <p class="hint">{i18n.t.routing.hint}</p>
    <table>
      <thead>
        <tr>
          <th scope="col" class="label">{i18n.t.routing.source}</th>
          {#each ROUTING_OUTPUTS as output (output)}
            <th scope="col">{i18n.t.routing.outputs[output]}</th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each rows as row (row.input)}
          <tr>
            <th scope="row">{i18n.t.routing.inputs[row.input]}</th>
            {#each ROUTING_OUTPUTS as output (output)}
              {@const possible = canRoute(row.input, output)}
              {@const on = possible && row.outputs.includes(output)}
              <td>
                <!-- A button, not an input: it shows what the device says, never the click. -->
                <button
                  type="button"
                  role="checkbox"
                  aria-checked={on}
                  aria-label={label(row, output)}
                  disabled={!possible}
                  onclick={() => {
                    if (possible) {
                      sendIntent({ type: 'setRoute', input: row.input, output, on: !on });
                    }
                  }}
                >
                  {#if !possible}
                    <span aria-hidden="true">–</span>
                  {:else if on}
                    <svg viewBox="0 0 16 16" aria-hidden="true">
                      <path d="M3 8.5 6.5 12 13 4.5" />
                    </svg>
                  {/if}
                </button>
              </td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
    {#if monitorDown}
      <p class="hint" role="note">{i18n.t.routing.monitorHint}</p>
    {/if}
    <p class="hint">{i18n.t.routing.loopHint}</p>
    <dl>
      {#each ROUTING_OUTPUTS as output (output)}
        <dt class="label">{i18n.t.routing.outputs[output]}</dt>
        <dd>{i18n.t.routing.outputHints[output]}</dd>
      {/each}
    </dl>
  {:else}
    <p class="hint">{i18n.t.mixer.connecting}</p>
  {/if}
</section>

<style>
  section {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 14px;
  }

  .hint {
    color: var(--legend);
  }

  table {
    margin: 18px 0 4px;
    border-collapse: separate;
    border-spacing: 0;
    background: var(--panel);
    border: 1px solid var(--unlit);
    border-radius: 8px;
  }

  th,
  td {
    padding: 8px 14px;
    text-align: center;
  }

  thead th {
    padding-top: 14px;
    padding-bottom: 12px;
    font-size: 13px;
    font-weight: 600;
    line-height: 1.25;
    max-width: 104px;
  }

  th[scope='row'],
  thead th:first-child {
    padding-left: 18px;
    padding-right: 28px;
    text-align: left;
  }

  th[scope='row'] {
    font-weight: 600;
    white-space: nowrap;
  }

  tbody tr > * {
    border-top: 1px solid var(--unlit);
  }

  tbody tr:hover > * {
    background: color-mix(in srgb, var(--unlit) 30%, transparent);
  }

  button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    margin: 0 auto;
    padding: 0;
    border: 1px solid var(--legend);
    border-radius: 5px;
    background: none;
    color: var(--legend);
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    border-color: var(--silkscreen);
  }

  /* A route that is on is filled and carries a tick: never colour alone. */
  button[aria-checked='true'] {
    background: var(--amber);
    border-color: var(--amber);
    color: var(--case);
  }

  button:disabled {
    border-color: transparent;
    cursor: default;
  }

  svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 18px;
    align-items: baseline;
    margin: 14px 0 0;
    padding-top: 20px;
    border-top: 1px solid var(--unlit);
  }

  dd {
    margin: 0;
    color: var(--legend);
    font-size: 13px;
  }
</style>
