<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { marketState, actions } from '../store';

  export let selectedStockId: string;

  const dispatch = createEventDispatcher();

  let orderType: 'market' | 'limit' = 'limit';
  let side: 'buy' | 'sell' = 'buy';
  let volume: number = 100;
  let price: number = 0;
  let isSubmitting: boolean = false;

  // Get current market data for smart defaults
  $: currentOrderBook = $marketState?.market_state?.order_books?.[selectedStockId];
  $: bestBid = currentOrderBook ? Math.max(...Object.keys(currentOrderBook.bids || {}).map(p => parseFloat(p))) / 100 : 0;
  $: bestAsk = currentOrderBook ? Math.min(...Object.keys(currentOrderBook.asks || {}).map(p => parseFloat(p))) / 100 : 0;
  $: midPrice = (bestBid + bestAsk) / 2;
  $: spread = bestAsk - bestBid;
  $: lastPrice = $marketState?.market_state?.last_traded_price?.[selectedStockId] || 0;

  // Auto-set price based on side and order type
  $: if (orderType === 'limit' && price === 0) {
    if (side === 'buy' && bestBid > 0) {
      price = bestBid;
    } else if (side === 'sell' && bestAsk > 0) {
      price = bestAsk;
    } else if (lastPrice > 0) {
      price = lastPrice;
    }
  }

  function handleQuickVolume(vol: number) {
    volume = vol;
  }

  function handleQuickPrice(priceType: 'bid' | 'ask' | 'mid' | 'last') {
    switch (priceType) {
      case 'bid':
        price = bestBid;
        break;
      case 'ask':
        price = bestAsk;
        break;
      case 'mid':
        price = midPrice;
        break;
      case 'last':
        price = lastPrice;
        break;
    }
  }

  async function submitOrder() {
    if (isSubmitting || volume <= 0) return;
    
    if (orderType === 'limit' && price <= 0) {
      alert('Please enter a valid price for limit orders');
      return;
    }

    isSubmitting = true;
    
    try {
      actions.submitOrder({
        client_id: 'web_trader',
        stock_id: parseInt(selectedStockId),
        side: side === 'buy' ? 'Buy' : 'Sell', // Rust expects "Buy"/"Sell", not "BUY"/"SELL"
        order_type: orderType === 'market' ? 'Market' : 'Limit', // Rust expects "Market"/"Limit"
        volume: volume,
        price: orderType === 'limit' ? price : 0
      });
      
      // Reset form on successful submission
      volume = 100;
      if (orderType === 'limit') {
        price = side === 'buy' ? bestBid : bestAsk;
      }
    } catch (error) {
      console.error('Failed to submit order:', error);
      alert('Failed to submit order. Please try again.');
    } finally {
      isSubmitting = false;
    }
  }

  function handleKeyPress(event: KeyboardEvent) {
    if (event.key === 'Enter' && !isSubmitting) {
      submitOrder();
    }
  }
</script>

<div class="trading-panel">
  <div class="panel-header">
    <h3>⚡ Quick Trade</h3>
    <div class="market-info">
      {#if bestBid > 0 && bestAsk > 0}
        <span class="spread-info">
          Spread: <span class="spread-value">${spread.toFixed(2)}</span>
        </span>
      {/if}
    </div>
  </div>

  <div class="order-form">
    <!-- Order Type Toggle -->
    <div class="order-type-toggle">
      <button 
        class="toggle-btn" 
        class:active={orderType === 'market'}
        on:click={() => orderType = 'market'}
      >
        Market
      </button>
      <button 
        class="toggle-btn" 
        class:active={orderType === 'limit'}
        on:click={() => orderType = 'limit'}
      >
        Limit
      </button>
    </div>

    <!-- Side Toggle -->
    <div class="side-toggle">
      <button 
        class="side-btn buy-btn" 
        class:active={side === 'buy'}
        on:click={() => side = 'buy'}
      >
        🟢 BUY
      </button>
      <button 
        class="side-btn sell-btn" 
        class:active={side === 'sell'}
        on:click={() => side = 'sell'}
      >
        🔴 SELL
      </button>
    </div>

    <!-- Volume Input -->
    <div class="input-group">
      <label for="volume-input">Volume</label>
      <input 
        id="volume-input"
        type="number" 
        bind:value={volume} 
        min="1" 
        step="1"
        on:keypress={handleKeyPress}
      />
      <div class="quick-volumes">
        {#each [100, 500, 1000, 5000] as vol}
          <button class="quick-btn" on:click={() => handleQuickVolume(vol)}>
            {vol.toLocaleString()}
          </button>
        {/each}
      </div>
    </div>

    <!-- Price Input (for limit orders) -->
    {#if orderType === 'limit'}
      <div class="input-group">
        <label for="price-input">Price ($)</label>
        <input 
          id="price-input"
          type="number" 
          bind:value={price} 
          min="0.01" 
          step="0.01"
          on:keypress={handleKeyPress}
        />
        <div class="quick-prices">
          <button class="quick-btn" on:click={() => handleQuickPrice('bid')}>
            Bid ${bestBid.toFixed(2)}
          </button>
          <button class="quick-btn" on:click={() => handleQuickPrice('mid')}>
            Mid ${midPrice.toFixed(2)}
          </button>
          <button class="quick-btn" on:click={() => handleQuickPrice('ask')}>
            Ask ${bestAsk.toFixed(2)}
          </button>
        </div>
      </div>
    {/if}

    <!-- Submit Button -->
    <button 
      class="submit-btn"
      class:buy-submit={side === 'buy'}
      class:sell-submit={side === 'sell'}
      disabled={isSubmitting || volume <= 0}
      on:click={submitOrder}
    >
      {#if isSubmitting}
        ⏳ Submitting...
      {:else}
        {side === 'buy' ? '🚀 BUY' : '📉 SELL'} {volume.toLocaleString()} @ {orderType === 'market' ? 'Market' : `$${price.toFixed(2)}`}
      {/if}
    </button>

    <!-- Order Summary -->
    <div class="order-summary">
      <div class="summary-row">
        <span>Estimated Cost:</span>
        <span class="summary-value">
          ${((orderType === 'market' ? (side === 'buy' ? bestAsk : bestBid) : price) * volume).toLocaleString()}
        </span>
      </div>
      {#if orderType === 'limit'}
        <div class="summary-row">
          <span>Distance from Mid:</span>
          <span class="summary-value" class:positive={price > midPrice} class:negative={price < midPrice}>
            {((price - midPrice) / midPrice * 100).toFixed(2)}%
          </span>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .trading-panel {
    background: var(--bg-secondary, #1a1f2e);
    border-radius: 8px;
    border: 1px solid var(--border-color, #2a2e39);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    overflow: hidden;
    height: fit-content;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    background: var(--bg-tertiary, #252a3a);
    border-bottom: 1px solid var(--border-color, #2a2e39);
  }

  .panel-header h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary, #d1d4dc);
  }

  .market-info {
    font-size: 12px;
    color: var(--text-secondary, #848e9c);
  }

  .spread-value {
    color: var(--accent-blue, #42a5f5);
    font-weight: 600;
  }

  .order-form {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .order-type-toggle, .side-toggle {
    display: flex;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid var(--border-color, #2a2e39);
  }

  .toggle-btn, .side-btn {
    flex: 1;
    padding: 8px 12px;
    background: var(--bg-tertiary, #252a3a);
    color: var(--text-secondary, #848e9c);
    border: none;
    cursor: pointer;
    font-size: 12px;
    font-weight: 600;
    transition: all 0.2s ease;
  }

  .toggle-btn:hover, .side-btn:hover {
    background: var(--border-color, #2a2e39);
  }

  .toggle-btn.active {
    background: var(--accent-blue, #42a5f5);
    color: white;
  }

  .buy-btn.active {
    background: var(--accent-green, #26a69a);
    color: white;
  }

  .sell-btn.active {
    background: var(--accent-red, #ef5350);
    color: white;
  }

  .input-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .input-group label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary, #848e9c);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .input-group input {
    padding: 10px 12px;
    background: var(--bg-tertiary, #252a3a);
    border: 1px solid var(--border-color, #2a2e39);
    border-radius: 6px;
    color: var(--text-primary, #d1d4dc);
    font-size: 14px;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
  }

  .input-group input:focus {
    outline: none;
    border-color: var(--accent-blue, #42a5f5);
    box-shadow: 0 0 0 2px rgba(66, 165, 245, 0.2);
  }

  .quick-volumes, .quick-prices {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }

  .quick-btn {
    padding: 4px 8px;
    background: var(--bg-primary, #0f1419);
    border: 1px solid var(--border-color, #2a2e39);
    border-radius: 4px;
    color: var(--text-secondary, #848e9c);
    font-size: 10px;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .quick-btn:hover {
    background: var(--border-color, #2a2e39);
    color: var(--text-primary, #d1d4dc);
  }

  .submit-btn {
    padding: 12px 16px;
    border: none;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .submit-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .buy-submit {
    background: var(--accent-green, #26a69a);
    color: white;
  }

  .buy-submit:hover:not(:disabled) {
    background: #2baf9a;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(38, 166, 154, 0.3);
  }

  .sell-submit {
    background: var(--accent-red, #ef5350);
    color: white;
  }

  .sell-submit:hover:not(:disabled) {
    background: #f26c6c;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(239, 83, 80, 0.3);
  }

  .order-summary {
    padding: 12px;
    background: var(--bg-primary, #0f1419);
    border-radius: 6px;
    border: 1px solid var(--border-color, #2a2e39);
  }

  .summary-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 12px;
    margin-bottom: 4px;
  }

  .summary-row:last-child {
    margin-bottom: 0;
  }

  .summary-row span:first-child {
    color: var(--text-secondary, #848e9c);
  }

  .summary-value {
    font-weight: 600;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
    color: var(--text-primary, #d1d4dc);
  }

  .summary-value.positive {
    color: var(--accent-green, #26a69a);
  }

  .summary-value.negative {
    color: var(--accent-red, #ef5350);
  }
</style>