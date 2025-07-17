<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { TimeFrame } from '../../lib/types';
  import { connectionStatus } from '../store';

  export let stockMap: Map<string, { ticker: string; company_name: string }> = new Map();
  export let selectedStockId: string;
  export let selectedTimeFrame: TimeFrame;
  export let isCandlestick: boolean;
  export let isDarkTheme: boolean;
  export let showDepthChart: boolean;

  const dispatch = createEventDispatcher();

  function handleStockChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    dispatch('stockChange', { stockId: target.value });
  }

  function handleTimeframeChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    dispatch('timeframeChange', { timeframe: target.value });
  }

  $: connectionStatusColor = {
    'connected': '#26a69a',
    'connecting': '#ff9800',
    'disconnected': '#ef5350'
  }[$connectionStatus] || '#848e9c';

  $: connectionStatusIcon = {
    'connected': '🟢',
    'connecting': '🟡',
    'disconnected': '🔴'
  }[$connectionStatus] || '⚪';
</script>

<header class="app-header">
  <div class="logo-area">
    <span class="logo-icon">🚀</span>
    <span class="logo-text">CHADSDAQ</span>
    <span class="logo-subtitle">Professional Trading Platform</span>
  </div>

  <div class="center-controls">
    <div class="stock-selector">
      {#if stockMap.size > 0}
        <label for="symbol-select">Symbol:</label>
        <select id="symbol-select" on:change={handleStockChange} value={selectedStockId} class="symbol-select">
          {#each Array.from(stockMap.entries()) as [id, stock] (id)}
            <option value={id}>{stock.ticker}</option>
          {/each}
        </select>
      {/if}
    </div>

    <div class="timeframe-selector">
      <label for="timeframe-select">Timeframe:</label>
      <select id="timeframe-select" on:change={handleTimeframeChange} value={selectedTimeFrame} class="timeframe-select">
        {#each Object.values(TimeFrame) as tf}
          <option value={tf}>{tf}</option>
        {/each}
      </select>
    </div>
  </div>

  <div class="right-controls">
    <div class="connection-status" style="color: {connectionStatusColor}">
      <span class="status-icon">{connectionStatusIcon}</span>
      <span class="status-text">{$connectionStatus.toUpperCase()}</span>
    </div>

    <div class="control-group">
      <button 
        class="control-btn" 
        class:active={isCandlestick}
        on:click={() => dispatch('chartTypeToggle')}
        title="Toggle Chart Type (Ctrl+C)"
      >
        {isCandlestick ? '🕯️' : '📈'}
      </button>

      <button 
        class="control-btn" 
        class:active={showDepthChart}
        on:click={() => dispatch('depthToggle')}
        title="Toggle Depth Chart (Ctrl+D)"
      >
        📊
      </button>

      <button 
        class="control-btn theme-btn" 
        on:click={() => dispatch('themeToggle')}
        title="Toggle Theme"
      >
        {isDarkTheme ? '☀️' : '🌙'}
      </button>
    </div>

    <a href="/portfolio" class="nav-link">
      📋 Portfolio
    </a>
  </div>
</header>

<style>
  .app-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 20px;
    height: 60px;
    background: linear-gradient(135deg, var(--bg-tertiary, #252a3a) 0%, var(--bg-secondary, #1a1f2e) 100%);
    border-bottom: 2px solid var(--border-color, #2a2e39);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
    flex-shrink: 0;
  }

  .logo-area {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .logo-icon {
    font-size: 28px;
    filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.3));
  }

  .logo-text {
    font-size: 20px;
    font-weight: 800;
    color: var(--text-primary, #d1d4dc);
    letter-spacing: 1px;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
  }

  .logo-subtitle {
    font-size: 10px;
    color: var(--text-secondary, #848e9c);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-left: 8px;
    opacity: 0.8;
  }

  .center-controls {
    display: flex;
    align-items: center;
    gap: 24px;
  }

  .stock-selector, .timeframe-selector {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .stock-selector label, .timeframe-selector label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary, #848e9c);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .symbol-select, .timeframe-select {
    background: var(--bg-primary, #0f1419);
    color: var(--text-primary, #d1d4dc);
    border: 1px solid var(--border-color, #2a2e39);
    border-radius: 6px;
    padding: 8px 12px;
    font-size: 13px;
    font-weight: 600;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .symbol-select:hover, .timeframe-select:hover {
    border-color: var(--accent-blue, #42a5f5);
    box-shadow: 0 0 0 2px rgba(66, 165, 245, 0.2);
  }

  .symbol-select:focus, .timeframe-select:focus {
    outline: none;
    border-color: var(--accent-blue, #42a5f5);
    box-shadow: 0 0 0 3px rgba(66, 165, 245, 0.3);
  }

  .right-controls {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .connection-status {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    background: var(--bg-primary, #0f1419);
    border-radius: 20px;
    border: 1px solid var(--border-color, #2a2e39);
  }

  .status-icon {
    font-size: 12px;
  }

  .status-text {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.5px;
  }

  .control-group {
    display: flex;
    align-items: center;
    gap: 4px;
    background: var(--bg-primary, #0f1419);
    border-radius: 8px;
    padding: 4px;
    border: 1px solid var(--border-color, #2a2e39);
  }

  .control-btn {
    background: transparent;
    color: var(--text-secondary, #848e9c);
    border: none;
    border-radius: 6px;
    padding: 8px 10px;
    font-size: 16px;
    cursor: pointer;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .control-btn:hover {
    background: var(--bg-tertiary, #252a3a);
    color: var(--text-primary, #d1d4dc);
    transform: translateY(-1px);
  }

  .control-btn.active {
    background: var(--accent-blue, #42a5f5);
    color: white;
    box-shadow: 0 2px 8px rgba(66, 165, 245, 0.3);
  }

  .theme-btn:hover {
    background: var(--accent-blue, #42a5f5);
    color: white;
  }

  .nav-link {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    background: var(--bg-primary, #0f1419);
    color: var(--text-primary, #d1d4dc);
    text-decoration: none;
    border-radius: 6px;
    border: 1px solid var(--border-color, #2a2e39);
    font-size: 12px;
    font-weight: 600;
    transition: all 0.2s ease;
  }

  .nav-link:hover {
    background: var(--accent-blue, #42a5f5);
    color: white;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(66, 165, 245, 0.3);
  }

  /* Responsive adjustments */
  @media (max-width: 1200px) {
    .logo-subtitle {
      display: none;
    }
    
    .center-controls {
      gap: 16px;
    }
  }

  @media (max-width: 900px) {
    .app-header {
      padding: 0 12px;
    }
    
    .center-controls {
      gap: 12px;
    }
    
    .right-controls {
      gap: 8px;
    }
  }
</style>