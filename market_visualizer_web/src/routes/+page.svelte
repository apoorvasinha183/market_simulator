<script lang="ts">
  import { onMount } from 'svelte';
  import Header from '$lib/components/Header.svelte';
  import PriceChart from '$lib/components/PriceChart.svelte';
  import OrderBook from '$lib/components/OrderBook.svelte';
  import DepthChart from '$lib/components/DepthChart.svelte';
  import OrderEntry from '$lib/components/OrderEntry.svelte';
  import TradeHistory from '$lib/components/TradeHistory.svelte';
  import MarketInfoBar from '$lib/components/MarketInfoBar.svelte';
  import TradingPanel from '$lib/components/TradingPanel.svelte';
  import { marketState, actions } from '../lib/store';
  import { TimeFrame, type Stock } from '../lib/types';

  // --- Component State ---
  let selectedStockId: string = '1';
  let selectedTimeFrame: TimeFrame = TimeFrame.TenSeconds;
  let showCandlestickChart: boolean = true;
  let stockMap: Map<string, Stock> = new Map();
  let isDarkTheme: boolean = true;
  let showDepthChart: boolean = true;

  // --- Lifecycle ---

  // --- Reactive Derivations ---
  $: {
    if ($marketState?.market_state?.stocks?.stocks) {
      const newStockMap = new Map<string, Stock>();
      $marketState.market_state.stocks.stocks.forEach(s => newStockMap.set(s.id.toString(), s));
      stockMap = newStockMap;
      if (!stockMap.has(selectedStockId) && stockMap.size > 0) {
        selectedStockId = stockMap.keys().next().value;
      }
    }
  }

  // --- Event Handlers ---
  function handleStockChange(event: CustomEvent<{ stockId: string }>) {
    selectedStockId = event.detail.stockId;
  }

  function handleTimeFrameChange(event: CustomEvent<{ timeframe: TimeFrame }>) {
    selectedTimeFrame = event.detail.timeframe;
  }

  function handleChartTypeToggle() {
    showCandlestickChart = !showCandlestickChart;
  }

  function toggleTheme() {
    isDarkTheme = !isDarkTheme;
    // Store theme preference in localStorage
    localStorage.setItem('darkTheme', isDarkTheme.toString());
  }

  // Load theme preference on mount
  onMount(() => {
    const savedTheme = localStorage.getItem('darkTheme');
    if (savedTheme !== null) {
      isDarkTheme = savedTheme === 'true';
    }
    actions.connect();
    
    // Keyboard shortcuts for professional traders
    const handleKeyPress = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        switch (e.key) {
          case 'b':
            e.preventDefault();
            // Focus buy order entry
            break;
          case 's':
            e.preventDefault();
            // Focus sell order entry
            break;
          case 'd':
            e.preventDefault();
            showDepthChart = !showDepthChart;
            break;
        }
      }
    };
    
    window.addEventListener('keydown', handleKeyPress);
    return () => window.removeEventListener('keydown', handleKeyPress);
  });

  function toggleDepthChart() {
    showDepthChart = !showDepthChart;
  }
</script>

<div class="trading-cockpit" class:dark-theme={isDarkTheme}>
  <Header 
    {stockMap} 
    {selectedStockId} 
    {selectedTimeFrame}
    {isDarkTheme}
    on:stockChange={handleStockChange}
    on:timeframeChange={handleTimeFrameChange}
    on:chartTypeToggle={handleChartTypeToggle}
    on:themeToggle={toggleTheme}
    on:depthToggle={toggleDepthChart}
    isCandlestick={showCandlestickChart}
    {showDepthChart}
  />

  <MarketInfoBar marketState={$marketState?.market_state} {selectedStockId} />

  <main class="main-content">
    <div class="left-panel">
      <div class="chart-container">
        <PriceChart 
          {selectedStockId} 
          {selectedTimeFrame}
          candleData={$marketState?.candle_data || {}}
          priceHistoryData={$marketState?.price_history || {}}
          isCandlestick={showCandlestickChart}
        />
      </div>
      {#if showDepthChart}
        <div class="depth-container">
          <DepthChart 
            marketState={$marketState?.market_state} 
            {selectedStockId}
          />
        </div>
      {/if}
    </div>
    
    <div class="center-panel">
      <OrderBook marketState={$marketState?.market_state} {selectedStockId} />
    </div>
    
    <div class="right-panel">
      <TradingPanel {selectedStockId} />
      <TradeHistory />
    </div>
  </main>
</div>

<style>
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen,
      Ubuntu, Cantarell, 'Open Sans', 'Helvetica Neue', sans-serif;
    background-color: #131722;
    color: #d1d4dc;
  }

  .trading-cockpit {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
    background: linear-gradient(135deg, #0f1419 0%, #1a1f2e 100%);
  }

  .trading-cockpit.dark-theme {
    --bg-primary: #0f1419;
    --bg-secondary: #1a1f2e;
    --bg-tertiary: #252a3a;
    --border-color: #2a2e39;
    --text-primary: #d1d4dc;
    --text-secondary: #848e9c;
    --accent-green: #26a69a;
    --accent-red: #ef5350;
    --accent-blue: #42a5f5;
  }

  .trading-cockpit:not(.dark-theme) {
    --bg-primary: #ffffff;
    --bg-secondary: #f8f9fa;
    --bg-tertiary: #e9ecef;
    --border-color: #dee2e6;
    --text-primary: #212529;
    --text-secondary: #6c757d;
    --accent-green: #198754;
    --accent-red: #dc3545;
    --accent-blue: #0d6efd;
    background: linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%);
  }

  .main-content {
    flex-grow: 1;
    display: grid;
    grid-template-columns: 2fr 300px 280px;
    gap: 6px;
    padding: 6px;
    overflow: hidden;
    min-height: 0;
  }

  .left-panel {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }

  .chart-container {
    flex: 2;
    background: var(--bg-secondary, #1a1f2e);
    border-radius: 8px;
    border: 1px solid var(--border-color, #2a2e39);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    overflow: hidden;
  }

  .depth-container {
    flex: 1;
    background: var(--bg-secondary, #1a1f2e);
    border-radius: 8px;
    border: 1px solid var(--border-color, #2a2e39);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    overflow: hidden;
    min-height: 200px;
  }

  .center-panel {
    background: var(--bg-secondary, #1a1f2e);
    border-radius: 8px;
    border: 1px solid var(--border-color, #2a2e39);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    overflow: hidden;
  }

  .right-panel {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }

  /* Professional scrollbars */
  :global(::-webkit-scrollbar) {
    width: 6px;
    height: 6px;
  }

  :global(::-webkit-scrollbar-track) {
    background: var(--bg-primary, #0f1419);
  }

  :global(::-webkit-scrollbar-thumb) {
    background: var(--border-color, #2a2e39);
    border-radius: 3px;
  }

  :global(::-webkit-scrollbar-thumb:hover) {
    background: var(--text-secondary, #848e9c);
  }
</style>