<script>
  import { fly } from 'svelte/transition';
  import { hopList } from '$lib/stores';
  import { derived } from 'svelte/store';

  const latestLatency = derived(hopList, ($list) =>
    Object.fromEntries(
      $list.map((h) => [h.ip, (h.latencies || [])[Math.max(0, (h.latencies || []).length - 1)]])
    )
  );

  $: list = $hopList;
  $: last = $latestLatency;
</script>

<div class="card">
  <div class="title">Discovered Hops</div>
  {#if list.length === 0}
    <div style="color: var(--muted)">No hops yet. Start a trace or check permissions.</div>
  {:else}
    <ul class="hops">
      {#each list as hop (hop.ip)}
        <li class="hop-item" in:fly={{ y: -20, duration: 300 }}>
          <div class="hop-left">
            <div class="hop-ttl">{hop.hop}</div>
            <div>
              <div class="hop-host">{hop.hostname || hop.ip}</div>
              <div class="hop-ip">{hop.ip}</div>
            </div>
          </div>
          {#if last[hop.ip] === null}
            <div class="latency timeout">timeout</div>
          {:else if last[hop.ip] !== undefined}
            <div class="latency ok">{last[hop.ip]} ms</div>
          {:else}
            <div class="latency" style="color: var(--muted)">--</div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>
