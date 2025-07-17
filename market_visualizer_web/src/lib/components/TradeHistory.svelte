<script lang="ts">
  import { tradeHistory } from '$lib/store';

  function formatCurrency(value: number | undefined) {
    if (value === undefined || isNaN(value)) return 'N/A';
    return (value / 100).toLocaleString('en-US', { style: 'currency', currency: 'USD' });
  }
</script>

<div class="trade-history-container">
  <div class="header">
    <h3>📈 Recent Trades</h3>
    <div class="trade-count">
      {$tradeHistory.length} trades
    </div>
  </div>
  <div class="trade-list">
    {#if $tradeHistory.length > 0}
      <div class="trade-table">
        <div class="table-header">
          <div class="col-time">Time</div>
          <div class="col-side">Side</div>
          <div class="col-price">Price</div>
          <div class="col-volume">Volume</div>
        </div>
        <div class="table-body">
          {#each $tradeHistory as trade, index}
            <div class="trade-row" class:recent={index < 3}>
              <div class="col-time">
                {new Date().toLocaleTimeString('en-US', { 
                  hour12: false, 
                  hour: '2-digit', 
                  minute: '2-digit', 
                  second: '2-digit' 
                })}
              </div>
              <div class="col-side">
                <span class="side-badge" class:buy={trade.taker_side === 'Buy'} class:sell={trade.taker_side === 'Sell'}>
                  {trade.taker_side === 'Buy' ? '🟢 BUY' : '🔴 SELL'}
                </span>
              </div>
              <div class="col-price">
                ${(trade.price / 100).toFixed(2)}
              </div>
              <div class="col-volume">
                {trade.volume.toLocaleString()}
              </div>
            </div>
          {/each}
        </div>
      </div>
    {:else}
      <div class="empty-state">
        <div class="empty-icon">📊</div>
        <div class="empty-text">No trades yet</div>
        <div class="empty-subtext">Trade history will appear here</div>
      </div>
    {/if}
  </div>
</div>

<style>
  .trade-history-container {
    background: var(--bg-secondary, #1a1f2e);
    border-radius: 8px;
    border: 1px solid var(--border-color, #2a2e39);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    height: 100%;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    background: var(--bg-tertiary, #252a3a);
    border-bottom: 1px solid var(--border-color, #2a2e39);
  }

  .header h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary, #d1d4dc);
  }

  .trade-count {
    font-size: 12px;
    color: var(--text-secondary, #848e9c);
    font-weight: 600;
  }

  .trade-list {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .trade-table {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .table-header {
    display: grid;
    grid-template-columns: 60px 60px 1fr 60px;
    gap: 8px;
    padding: 8px 16px;
    background: var(--bg-primary, #0f1419);
    border-bottom: 1px solid var(--border-color, #2a2e39);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary, #848e9c);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .table-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
  }

  .trade-row {
    display: grid;
    grid-template-columns: 60px 60px 1fr 60px;
    gap: 8px;
    padding: 6px 16px;
    font-size: 11px;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
    border-bottom: 1px solid rgba(42, 46, 57, 0.3);
    transition: all 0.2s ease;
  }

  .trade-row:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .trade-row.recent {
    background: rgba(66, 165, 245, 0.1);
    border-left: 3px solid var(--accent-blue, #42a5f5);
  }

  .col-time, .col-side, .col-price, .col-volume {
    display: flex;
    align-items: center;
  }

  .col-time {
    color: var(--text-secondary, #848e9c);
    font-size: 10px;
  }

  .col-price, .col-volume {
    justify-content: flex-end;
    color: var(--text-primary, #d1d4dc);
    font-weight: 600;
  }

  .side-badge {
    display: inline-flex;
    align-items: center;
    padding: 2px 6px;
    border-radius: 12px;
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .side-badge.buy {
    background: rgba(38, 166, 154, 0.2);
    color: var(--accent-green, #26a69a);
    border: 1px solid rgba(38, 166, 154, 0.3);
  }

  .side-badge.sell {
    background: rgba(239, 83, 80, 0.2);
    color: var(--accent-red, #ef5350);
    border: 1px solid rgba(239, 83, 80, 0.3);
  }

  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 40px 20px;
    text-align: center;
  }

  .empty-icon {
    font-size: 32px;
    margin-bottom: 12px;
    opacity: 0.5;
  }

  .empty-text {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-secondary, #848e9c);
    margin-bottom: 4px;
  }

  .empty-subtext {
    font-size: 12px;
    color: var(--text-secondary, #848e9c);
    opacity: 0.7;
  }

  /* Scrollbar styling */
  .table-body::-webkit-scrollbar {
    width: 4px;
  }

  .table-body::-webkit-scrollbar-track {
    background: var(--bg-primary, #0f1419);
  }

  .table-body::-webkit-scrollbar-thumb {
    background: var(--border-color, #2a2e39);
    border-radius: 2px;
  }

  .table-body::-webkit-scrollbar-thumb:hover {
    background: var(--text-secondary, #848e9c);
  }
</style>