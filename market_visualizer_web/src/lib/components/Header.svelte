<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { TimeFrame } from '../../lib/types';

  export let stockMap: Map<string, { ticker: string; company_name: string }> = new Map();
  export let selectedStockId: string;
  export let selectedTimeFrame: TimeFrame;
  export let isCandlestick: boolean;

  const dispatch = createEventDispatcher();

  function handleStockChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    dispatch('stockChange', { stockId: target.value });
  }

  function handleTimeframeChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    dispatch('timeframeChange', { timeframe: target.value });
  }
</script>

<header class="app-header">
  <div class="logo-area">
    <span class="logo-icon">📈</span>
    <span class="logo-text">MarketSim</span>
  </div>
  <div class="stock-selector">
    {#if stockMap.size > 0}
      <select on:change={handleStockChange} value={selectedStockId}>
        {#each Array.from(stockMap.entries()) as [id, stock] (id)}
          <option value={id}>{stock.ticker} - {stock.company_name}</option>
        {/each}
      </select>
    {/if}
  </div>
  <div class="controls">
    <select on:change={handleTimeframeChange} value={selectedTimeFrame}>
      {#each Object.values(TimeFrame) as tf}
        <option value={tf}>{tf}</option>
      {/each}
    </select>
    <button on:click={() => dispatch('chartTypeToggle')}>
      {isCandlestick ? 'Line Chart' : 'Candlestick'}
    </button>
  </div>
</header>

<style>
  .app-header {
    display: flex;
    align-items: center;
    padding: 0 16px;
    height: 50px;
    background-color: #1c212e;
    border-bottom: 1px solid #2a2e39;
    flex-shrink: 0;
  }

  .logo-area {
    display: flex;
    align-items: center;
  }

  .logo-icon {
    font-size: 24px;
  }

  .logo-text {
    font-size: 18px;
    font-weight: bold;
    margin-left: 8px;
  }

  .stock-selector {
    margin-left: 32px;
  }

  select, button {
    background-color: #2a2e39;
    color: #d1d4dc;
    border: 1px solid #4a4e59;
    border-radius: 4px;
    padding: 8px 12px;
    font-size: 14px;
  }

  .controls {
    margin-left: auto;
    display: flex;
    gap: 16px;
  }
</style>