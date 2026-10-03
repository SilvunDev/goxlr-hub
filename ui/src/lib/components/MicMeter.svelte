<script lang="ts">
  import { i18n } from '../i18n/index.svelte';

  let { levelDb }: { levelDb: number } = $props();

  /** Silence, as the device reports it. */
  const SILENCE_DB = -72.2;
  /** The meter starts moving at this level; anything below draws nothing. */
  const FLOOR_DB = -60;

  let level = $derived(
    typeof levelDb === 'number' && Number.isFinite(levelDb)
      ? Math.min(0, Math.max(SILENCE_DB, levelDb))
      : SILENCE_DB,
  );
  let percent = $derived(Math.max(0, ((level - FLOOR_DB) / -FLOOR_DB) * 100));
  let text = $derived(
    `${level.toLocaleString(i18n.locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 })} dB`,
  );
</script>

<section class="card">
  <span class="label">{i18n.t.mixer.micLevel}</span>
  <div
    class="meter"
    role="meter"
    aria-label={i18n.t.mixer.micLevel}
    aria-valuemin={SILENCE_DB}
    aria-valuemax="0"
    aria-valuenow={level}
    aria-valuetext={text}
  >
    <div class="fill" style:width="{percent}%"></div>
  </div>
  <span class="value">{text}</span>
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .meter {
    position: relative;
    height: 28px;
    background: radial-gradient(circle, var(--unlit) 2.5px, transparent 3px) left top / 14px 14px;
  }

  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: radial-gradient(circle, var(--amber) 2.5px, transparent 3px) left top / 14px 14px;
    transition: width 60ms linear;
  }

  .value {
    font-family: var(--font-mono);
    font-size: 20px;
    font-variant-numeric: tabular-nums;
  }
</style>
