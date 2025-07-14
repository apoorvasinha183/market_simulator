
<script lang="ts">
  import type { MarketState } from '../../lib/types';

  export let marketState: MarketState | null;
  export let selectedStockId: string;

  $: lastTradedPrice = marketState?.last_traded_price?.[selectedStockId] ?? null;
  $: cumulativeVolume = marketState?.cumulative_volume?.[selectedStockId] ?? null;
  $: midPrice = marketState?.mid_prices?.[selectedStockId] ?? null;
  $: spread = marketState?.spreads?.[selectedStockId] ?? null;

  function formatNumber(num: number | null): string {
    if (num === null || num === undefined) return 'N/A';
    return num.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  }
</script>

<div class="market-summary-bar">
  <div class="summary-item">
    <span class="label">Last Price</span>
    <span class="value">${formatNumber(lastTradedPrice)}</span>
  </div>
  <div class="summary-item">
    <span class="label">Volume</span>
    <span class="value">{cumulativeVolume?.toLocaleString() || 'N/A'}</span>
  </div>
  <div class="summary-item">
    <span class="label">Mid Price</span>
    <span class="value">${formatNumber(midPrice)}</span>
  </div>
  <div class="summary-item">
    <span class="label">Spread</span>
    <span class="value">${formatNumber(spread)}</span>
  </div>
</div>

<style>
  .market-summary-bar {
    display: flex;
    justify-content: space-around;
    padding: 8px;
    background-color: #1c212e;
    border-bottom: 1px solid #2a2e39;
  }

  .summary-item {
    text-align: center;
  }

  .label {
    font-size: 12px;
    color: #848e9c;
    display: block;
  }

  .value {
    font-size: 16px;
    font-weight: bold;
  }
</style>
