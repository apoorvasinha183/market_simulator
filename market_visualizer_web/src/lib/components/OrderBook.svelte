
<script lang="ts">
  import type { MarketState } from '../../lib/types';

  export let marketState: MarketState | null;
  export let selectedStockId: string;

  $: orderBook = marketState?.order_books?.[selectedStockId];
  $: bids = orderBook ? Object.entries(orderBook.bids).map(([price, level]) => ({ 
    price: parseFloat(price) / 100, 
    volume: level.total_volume,
    orders: level.orders?.length || 0
  })).sort((a, b) => b.price - a.price).slice(0, 15) : [];
  
  $: asks = orderBook ? Object.entries(orderBook.asks).map(([price, level]) => ({ 
    price: parseFloat(price) / 100, 
    volume: level.total_volume,
    orders: level.orders?.length || 0
  })).sort((a, b) => a.price - b.price).slice(0, 15) : [];

  $: maxVolume = Math.max(
    ...(bids.map(b => b.volume)),
    ...(asks.map(a => a.volume)),
    1
  );

  $: bestBid = bids[0]?.price || 0;
  $: bestAsk = asks[0]?.price || 0;
  $: spread = bestAsk - bestBid;
  $: spreadPercent = bestBid > 0 ? (spread / bestBid) * 100 : 0;

  function formatVolume(volume: number): string {
    if (volume >= 1000000) {
      return (volume / 1000000).toFixed(1) + 'M';
    } else if (volume >= 1000) {
      return (volume / 1000).toFixed(1) + 'K';
    }
    return volume.toLocaleString();
  }
</script>

<div class="order-book-container">
  <div class="header">
    <div class="header-title">
      <h3>📊 Order Book</h3>
      <div class="book-stats">
        <span class="stat">
          <span class="stat-label">Spread:</span>
          <span class="spread-value">${spread.toFixed(2)} ({spreadPercent.toFixed(2)}%)</span>
        </span>
      </div>
    </div>
    <div class="column-headers">
      <div class="col-header price-col">Price</div>
      <div class="col-header volume-col">Volume</div>
      <div class="col-header orders-col">#</div>
    </div>
  </div>

  <div class="book-body">
    <!-- Asks Section -->
    <div class="asks-section">
      <div class="section-label asks-label">📈 ASKS</div>
      <div class="book-rows">
        {#each asks as ask, index (ask.price)}
          <div class="book-row ask-row" style="--depth: {ask.volume / maxVolume}">
            <div class="depth-bar ask-depth"></div>
            <div class="price-cell ask-price">${ask.price.toFixed(2)}</div>
            <div class="volume-cell">{formatVolume(ask.volume)}</div>
            <div class="orders-cell">{ask.orders}</div>
          </div>
        {/each}
        {#if asks.length === 0}
          <div class="empty-state">No asks</div>
        {/if}
      </div>
    </div>

    <!-- Spread Indicator -->
    <div class="spread-indicator">
      <div class="spread-line"></div>
      <div class="spread-info">
        <span class="spread-label">SPREAD</span>
        <span class="spread-amount">${spread.toFixed(2)}</span>
      </div>
    </div>

    <!-- Bids Section -->
    <div class="bids-section">
      <div class="section-label bids-label">📉 BIDS</div>
      <div class="book-rows">
        {#each bids as bid, index (bid.price)}
          <div class="book-row bid-row" style="--depth: {bid.volume / maxVolume}">
            <div class="depth-bar bid-depth"></div>
            <div class="price-cell bid-price">${bid.price.toFixed(2)}</div>
            <div class="volume-cell">{formatVolume(bid.volume)}</div>
            <div class="orders-cell">{bid.orders}</div>
          </div>
        {/each}
        {#if bids.length === 0}
          <div class="empty-state">No bids</div>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .order-book-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-secondary, #1a1f2e);
    overflow: hidden;
  }

  .header {
    background: var(--bg-tertiary, #252a3a);
    border-bottom: 1px solid var(--border-color, #2a2e39);
    padding: 12px 16px;
  }

  .header-title {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }

  .header-title h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary, #d1d4dc);
  }

  .book-stats {
    font-size: 12px;
  }

  .stat-label {
    color: var(--text-secondary, #848e9c);
  }

  .spread-value {
    color: var(--accent-blue, #42a5f5);
    font-weight: 600;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
  }

  .column-headers {
    display: grid;
    grid-template-columns: 1fr 80px 30px;
    gap: 8px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary, #848e9c);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .col-header {
    text-align: right;
  }

  .price-col {
    text-align: left;
  }

  .book-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .asks-section, .bids-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .section-label {
    padding: 8px 16px;
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 1px;
    border-bottom: 1px solid var(--border-color, #2a2e39);
  }

  .asks-label {
    background: rgba(239, 83, 80, 0.1);
    color: var(--accent-red, #ef5350);
  }

  .bids-label {
    background: rgba(38, 166, 154, 0.1);
    color: var(--accent-green, #26a69a);
  }

  .book-rows {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
  }

  .book-row {
    position: relative;
    display: grid;
    grid-template-columns: 1fr 80px 30px;
    gap: 8px;
    padding: 3px 16px;
    font-size: 12px;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
    transition: background-color 0.1s ease;
  }

  .book-row:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .depth-bar {
    position: absolute;
    top: 0;
    right: 0;
    height: 100%;
    width: calc(var(--depth) * 100%);
    opacity: 0.15;
    z-index: 0;
    transition: opacity 0.2s ease;
  }

  .book-row:hover .depth-bar {
    opacity: 0.25;
  }

  .ask-depth {
    background: linear-gradient(90deg, transparent 0%, var(--accent-red, #ef5350) 100%);
  }

  .bid-depth {
    background: linear-gradient(90deg, transparent 0%, var(--accent-green, #26a69a) 100%);
  }

  .price-cell, .volume-cell, .orders-cell {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
  }

  .price-cell {
    justify-content: flex-start;
    font-weight: 600;
  }

  .volume-cell, .orders-cell {
    justify-content: flex-end;
  }

  .ask-price {
    color: var(--accent-red, #ef5350);
  }

  .bid-price {
    color: var(--accent-green, #26a69a);
  }

  .volume-cell {
    color: var(--text-primary, #d1d4dc);
  }

  .orders-cell {
    color: var(--text-secondary, #848e9c);
    font-size: 10px;
  }

  .spread-indicator {
    position: relative;
    padding: 8px 16px;
    background: var(--bg-primary, #0f1419);
    border-top: 1px solid var(--border-color, #2a2e39);
    border-bottom: 1px solid var(--border-color, #2a2e39);
  }

  .spread-line {
    height: 1px;
    background: linear-gradient(90deg, transparent 0%, var(--accent-blue, #42a5f5) 50%, transparent 100%);
    margin-bottom: 4px;
  }

  .spread-info {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 11px;
  }

  .spread-label {
    color: var(--text-secondary, #848e9c);
    font-weight: 600;
    letter-spacing: 0.5px;
  }

  .spread-amount {
    color: var(--accent-blue, #42a5f5);
    font-weight: 700;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
  }

  .empty-state {
    padding: 20px;
    text-align: center;
    color: var(--text-secondary, #848e9c);
    font-style: italic;
    font-size: 12px;
  }

  /* Scrollbar styling */
  .book-rows::-webkit-scrollbar {
    width: 4px;
  }

  .book-rows::-webkit-scrollbar-track {
    background: var(--bg-primary, #0f1419);
  }

  .book-rows::-webkit-scrollbar-thumb {
    background: var(--border-color, #2a2e39);
    border-radius: 2px;
  }

  .book-rows::-webkit-scrollbar-thumb:hover {
    background: var(--text-secondary, #848e9c);
  }
</style>
