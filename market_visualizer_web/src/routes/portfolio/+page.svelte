
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { portfolio, marketState, tradeHistory, actions } from '$lib/store';
  import { goto } from '$app/navigation';
  import type { Chart } from 'chart.js';

  let allocationChart: Chart | null = null;
  let performanceChart: Chart | null = null;
  let allocationCanvas: HTMLCanvasElement;
  let performanceCanvas: HTMLCanvasElement;

  // Reactive declarations for portfolio calculations
  $: currentPrices = $marketState?.market_state?.last_traded_price || {};
  $: stockMap = $marketState?.market_state?.stocks?.stocks || [];

  $: calculatedHoldings = $portfolio?.holdings ? Object.values($portfolio.holdings).map(holding => {
    const currentPrice = currentPrices[holding.stock_id] || 0;
    const holdingTotalValue = holding.quantity * currentPrice;
    const costBasis = holding.cost_basis * holding.quantity;
    const unrealizedPnL = holdingTotalValue - costBasis;
    const unrealizedPnLPercent = costBasis > 0 ? (unrealizedPnL / costBasis) * 100 : 0;
    const stock = stockMap.find(s => s.id === holding.stock_id);
    
    return { 
      ...holding, 
      currentPrice, 
      holdingTotalValue, 
      costBasis,
      unrealizedPnL,
      unrealizedPnLPercent,
      ticker: stock?.ticker || `STOCK_${holding.stock_id}`,
      companyName: stock?.company_name || 'Unknown Company'
    };
  }) : [];

  $: portfolioTotalValue = ($portfolio?.cash || 0) + calculatedHoldings.reduce((sum, h) => sum + h.holdingTotalValue, 0);
  $: totalCostBasis = calculatedHoldings.reduce((sum, h) => sum + h.costBasis, 0);
  $: totalUnrealizedPnL = calculatedHoldings.reduce((sum, h) => sum + h.unrealizedPnL, 0);
  $: totalUnrealizedPnLPercent = totalCostBasis > 0 ? (totalUnrealizedPnL / totalCostBasis) * 100 : 0;
  $: cashPercentage = portfolioTotalValue > 0 ? (($portfolio?.cash || 0) / portfolioTotalValue) * 100 : 0;

  // Performance metrics
  $: dayChange = 0; // TODO: Calculate from price history
  $: dayChangePercent = 0; // TODO: Calculate from price history

  onMount(async () => {
    actions.connect();
    
    // Trigger context change for portfolio page
    const portfolioStocks = calculatedHoldings.map(h => h.stock_id);
    actions.changeContext({
      page: 'portfolio',
      selected_stocks: portfolioStocks.length > 0 ? portfolioStocks : undefined
    });
    
    await initializeCharts();
    updateCharts();
  });

  onDestroy(() => {
    allocationChart?.destroy();
    performanceChart?.destroy();
  });

  async function initializeCharts() {
    const { Chart: ChartJS, registerables } = await import('chart.js');
    ChartJS.register(...registerables);

    // Asset Allocation Pie Chart
    if (allocationCanvas && !allocationChart) {
      const ctx = allocationCanvas.getContext('2d');
      if (ctx) {
        allocationChart = new ChartJS(ctx, {
          type: 'doughnut',
          data: { datasets: [] },
          options: {
            responsive: true,
            maintainAspectRatio: false,
            plugins: {
              legend: {
                position: 'right',
                labels: {
                  color: '#d1d4dc',
                  font: { size: 12 },
                  usePointStyle: true
                }
              },
              tooltip: {
                backgroundColor: 'rgba(26, 31, 46, 0.95)',
                titleColor: '#d1d4dc',
                bodyColor: '#d1d4dc',
                borderColor: '#2a2e39',
                borderWidth: 1,
                callbacks: {
                  label: function(context: any) {
                    const percentage = ((context.parsed / portfolioTotalValue) * 100).toFixed(1);
                    return `${context.label}: ${formatCurrency(context.parsed)} (${percentage}%)`;
                  }
                }
              }
            }
          }
        });
      }
    }

    // Performance Line Chart
    if (performanceCanvas && !performanceChart) {
      const ctx = performanceCanvas.getContext('2d');
      if (ctx) {
        performanceChart = new ChartJS(ctx, {
          type: 'line',
          data: { datasets: [] },
          options: {
            responsive: true,
            maintainAspectRatio: false,
            scales: {
              x: {
                type: 'time',
                grid: { color: 'rgba(255, 255, 255, 0.1)' },
                ticks: { color: '#d1d4dc' }
              },
              y: {
                grid: { color: 'rgba(255, 255, 255, 0.1)' },
                ticks: { 
                  color: '#d1d4dc',
                  callback: function(value: any) {
                    return formatCurrency(value as number);
                  }
                }
              }
            },
            plugins: {
              legend: {
                labels: { color: '#d1d4dc' }
              },
              tooltip: {
                backgroundColor: 'rgba(26, 31, 46, 0.95)',
                titleColor: '#d1d4dc',
                bodyColor: '#d1d4dc',
                borderColor: '#2a2e39',
                borderWidth: 1
              }
            }
          }
        });
      }
    }
  }

  function updateCharts() {
    if (allocationChart && calculatedHoldings.length > 0) {
      const labels = [...calculatedHoldings.map(h => h.ticker), 'Cash'];
      const data = [...calculatedHoldings.map(h => h.holdingTotalValue), $portfolio?.cash || 0];
      const colors = [
        '#26a69a', '#ef5350', '#42a5f5', '#ff9800', '#9c27b0',
        '#4caf50', '#f44336', '#2196f3', '#ff5722', '#673ab7',
        '#795548'
      ];

      allocationChart.data = {
        labels,
        datasets: [{
          data,
          backgroundColor: colors.slice(0, data.length),
          borderColor: '#2a2e39',
          borderWidth: 2
        }]
      };
      allocationChart.update('none');
    }

    // TODO: Update performance chart with historical portfolio values
  }

  // Update charts when data changes
  $: if (calculatedHoldings && allocationChart) {
    updateCharts();
  }

  function formatCurrency(value: number | undefined) {
    if (value === undefined || isNaN(value)) return '$0.00';
    return value.toLocaleString('en-US', { style: 'currency', currency: 'USD' });
  }

  function formatPercent(value: number) {
    return `${value >= 0 ? '+' : ''}${value.toFixed(2)}%`;
  }

  function goHome() {
    goto('/');
  }
</script>

<div class="portfolio-page">
  <!-- Header -->
  <header class="portfolio-header">
    <div class="header-left">
      <button class="back-button" on:click={goHome}>
        <span class="back-icon">←</span>
        <span>Back to Trading</span>
      </button>
      <h1>📊 Portfolio Dashboard</h1>
    </div>
    <div class="header-right">
      <div class="last-updated">
        Last updated: {new Date().toLocaleTimeString()}
      </div>
    </div>
  </header>

  <!-- Portfolio Overview Cards -->
  <div class="overview-grid">
    <div class="overview-card total-value">
      <div class="card-header">
        <h3>💰 Total Portfolio Value</h3>
      </div>
      <div class="card-content">
        <div class="main-value">{formatCurrency(portfolioTotalValue)}</div>
        <div class="sub-value" class:positive={dayChange >= 0} class:negative={dayChange < 0}>
          {formatPercent(dayChangePercent)} ({formatCurrency(dayChange)})
        </div>
      </div>
    </div>

    <div class="overview-card cash">
      <div class="card-header">
        <h3>💵 Available Cash</h3>
      </div>
      <div class="card-content">
        <div class="main-value">{formatCurrency($portfolio?.cash || 0)}</div>
        <div class="sub-value">{cashPercentage.toFixed(1)}% of portfolio</div>
      </div>
    </div>

    <div class="overview-card pnl">
      <div class="card-header">
        <h3>📈 Unrealized P&L</h3>
      </div>
      <div class="card-content">
        <div class="main-value" class:positive={totalUnrealizedPnL >= 0} class:negative={totalUnrealizedPnL < 0}>
          {formatCurrency(totalUnrealizedPnL)}
        </div>
        <div class="sub-value" class:positive={totalUnrealizedPnLPercent >= 0} class:negative={totalUnrealizedPnLPercent < 0}>
          {formatPercent(totalUnrealizedPnLPercent)}
        </div>
      </div>
    </div>

    <div class="overview-card positions">
      <div class="card-header">
        <h3>🎯 Active Positions</h3>
      </div>
      <div class="card-content">
        <div class="main-value">{calculatedHoldings.length}</div>
        <div class="sub-value">
          {calculatedHoldings.filter(h => h.unrealizedPnL >= 0).length} profitable
        </div>
      </div>
    </div>
  </div>

  <!-- Main Content Grid -->
  <div class="main-content">
    <!-- Holdings Table -->
    <div class="holdings-section">
      <div class="section-header">
        <h2>📋 Holdings</h2>
        <div class="section-controls">
          <span class="holdings-count">{calculatedHoldings.length} positions</span>
        </div>
      </div>
      
      <div class="holdings-table">
        {#if calculatedHoldings.length > 0}
          <div class="table-header">
            <div class="col-symbol">Symbol</div>
            <div class="col-quantity">Quantity</div>
            <div class="col-price">Avg Cost</div>
            <div class="col-price">Current</div>
            <div class="col-value">Market Value</div>
            <div class="col-pnl">Unrealized P&L</div>
            <div class="col-percent">%</div>
          </div>
          <div class="table-body">
            {#each calculatedHoldings as holding}
              <div class="table-row">
                <div class="col-symbol">
                  <div class="symbol-info">
                    <span class="ticker">{holding.ticker}</span>
                    <span class="company-name">{holding.companyName}</span>
                  </div>
                </div>
                <div class="col-quantity">{holding.quantity.toLocaleString()}</div>
                <div class="col-price">{formatCurrency(holding.cost_basis / holding.quantity)}</div>
                <div class="col-price">{formatCurrency(holding.currentPrice)}</div>
                <div class="col-value">{formatCurrency(holding.holdingTotalValue)}</div>
                <div class="col-pnl" class:positive={holding.unrealizedPnL >= 0} class:negative={holding.unrealizedPnL < 0}>
                  {formatCurrency(holding.unrealizedPnL)}
                </div>
                <div class="col-percent" class:positive={holding.unrealizedPnLPercent >= 0} class:negative={holding.unrealizedPnLPercent < 0}>
                  {formatPercent(holding.unrealizedPnLPercent)}
                </div>
              </div>
            {/each}
          </div>
        {:else}
          <div class="empty-state">
            <div class="empty-icon">📈</div>
            <div class="empty-title">No positions yet</div>
            <div class="empty-subtitle">Start trading to build your portfolio</div>
            <button class="start-trading-btn" on:click={goHome}>
              Start Trading
            </button>
          </div>
        {/if}
      </div>
    </div>

    <!-- Charts Section -->
    <div class="charts-section">
      <!-- Asset Allocation -->
      <div class="chart-card allocation-chart">
        <div class="chart-header">
          <h3>🥧 Asset Allocation</h3>
        </div>
        <div class="chart-content">
          <canvas bind:this={allocationCanvas}></canvas>
        </div>
      </div>

      <!-- Performance Chart -->
      <div class="chart-card performance-chart">
        <div class="chart-header">
          <h3>📊 Portfolio Performance</h3>
        </div>
        <div class="chart-content">
          <canvas bind:this={performanceCanvas}></canvas>
        </div>
      </div>

      <!-- Recent Activity -->
      <div class="activity-card">
        <div class="card-header">
          <h3>⚡ Recent Activity</h3>
        </div>
        <div class="activity-list">
          {#if $tradeHistory.length > 0}
            {#each $tradeHistory.slice(0, 5) as trade}
              <div class="activity-item">
                <div class="activity-icon" class:buy={trade.taker_side === 'Buy'} class:sell={trade.taker_side === 'Sell'}>
                  {trade.taker_side === 'Buy' ? '🟢' : '🔴'}
                </div>
                <div class="activity-details">
                  <div class="activity-main">
                    {trade.taker_side} {trade.volume} shares
                  </div>
                  <div class="activity-sub">
                    @ {formatCurrency(trade.price / 100)}
                  </div>
                </div>
                <div class="activity-time">
                  {new Date().toLocaleTimeString('en-US', { 
                    hour12: false, 
                    hour: '2-digit', 
                    minute: '2-digit' 
                  })}
                </div>
              </div>
            {/each}
          {:else}
            <div class="no-activity">
              <span class="no-activity-icon">📊</span>
              <span>No recent trades</span>
            </div>
          {/if}
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen,
      Ubuntu, Cantarell, 'Open Sans', 'Helvetica Neue', sans-serif;
    background: linear-gradient(135deg, #0f1419 0%, #1a1f2e 100%);
    color: #d1d4dc;
    min-height: 100vh;
  }

  .portfolio-page {
    min-height: 100vh;
    background: linear-gradient(135deg, #0f1419 0%, #1a1f2e 100%);
    color: #d1d4dc;
  }

  /* Header */
  .portfolio-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 24px;
    background: linear-gradient(135deg, #252a3a 0%, #1a1f2e 100%);
    border-bottom: 2px solid #2a2e39;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 20px;
  }

  .back-button {
    display: flex;
    align-items: center;
    gap: 8px;
    background: #0f1419;
    color: #d1d4dc;
    border: 1px solid #2a2e39;
    padding: 10px 16px;
    border-radius: 8px;
    cursor: pointer;
    font-size: 14px;
    font-weight: 600;
    transition: all 0.2s ease;
  }

  .back-button:hover {
    background: #42a5f5;
    color: white;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(66, 165, 245, 0.3);
  }

  .back-icon {
    font-size: 16px;
  }

  .portfolio-header h1 {
    margin: 0;
    font-size: 24px;
    font-weight: 800;
    color: #d1d4dc;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
  }

  .last-updated {
    font-size: 12px;
    color: #848e9c;
    font-weight: 500;
  }

  /* Overview Grid */
  .overview-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 20px;
    padding: 24px;
  }

  .overview-card {
    background: #1a1f2e;
    border-radius: 12px;
    border: 1px solid #2a2e39;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    overflow: hidden;
    transition: transform 0.2s ease;
  }

  .overview-card:hover {
    transform: translateY(-2px);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  }

  .card-header {
    padding: 16px 20px 0;
  }

  .card-header h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #848e9c;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .card-content {
    padding: 8px 20px 20px;
  }

  .main-value {
    font-size: 28px;
    font-weight: 800;
    color: #d1d4dc;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
    margin-bottom: 4px;
  }

  .sub-value {
    font-size: 14px;
    color: #848e9c;
    font-weight: 500;
  }

  .positive {
    color: #26a69a !important;
  }

  .negative {
    color: #ef5350 !important;
  }

  /* Main Content */
  .main-content {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 24px;
    padding: 0 24px 24px;
  }

  /* Holdings Section */
  .holdings-section {
    background: #1a1f2e;
    border-radius: 12px;
    border: 1px solid #2a2e39;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    overflow: hidden;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 24px;
    background: #252a3a;
    border-bottom: 1px solid #2a2e39;
  }

  .section-header h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
    color: #d1d4dc;
  }

  .holdings-count {
    font-size: 12px;
    color: #848e9c;
    font-weight: 600;
  }

  .holdings-table {
    overflow: hidden;
  }

  .table-header {
    display: grid;
    grid-template-columns: 2fr 1fr 1fr 1fr 1.2fr 1.2fr 0.8fr;
    gap: 16px;
    padding: 16px 24px;
    background: #0f1419;
    border-bottom: 1px solid #2a2e39;
    font-size: 12px;
    font-weight: 700;
    color: #848e9c;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .table-body {
    max-height: 400px;
    overflow-y: auto;
  }

  .table-row {
    display: grid;
    grid-template-columns: 2fr 1fr 1fr 1fr 1.2fr 1.2fr 0.8fr;
    gap: 16px;
    padding: 16px 24px;
    border-bottom: 1px solid rgba(42, 46, 57, 0.3);
    transition: background-color 0.2s ease;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
  }

  .table-row:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .symbol-info {
    display: flex;
    flex-direction: column;
  }

  .ticker {
    font-size: 14px;
    font-weight: 700;
    color: #d1d4dc;
  }

  .company-name {
    font-size: 11px;
    color: #848e9c;
    margin-top: 2px;
  }

  .col-quantity, .col-price, .col-value, .col-pnl, .col-percent {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    font-size: 13px;
    font-weight: 600;
  }

  /* Empty State */
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 60px 20px;
    text-align: center;
  }

  .empty-icon {
    font-size: 48px;
    margin-bottom: 16px;
    opacity: 0.5;
  }

  .empty-title {
    font-size: 18px;
    font-weight: 600;
    color: #d1d4dc;
    margin-bottom: 8px;
  }

  .empty-subtitle {
    font-size: 14px;
    color: #848e9c;
    margin-bottom: 24px;
  }

  .start-trading-btn {
    background: #26a69a;
    color: white;
    border: none;
    padding: 12px 24px;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .start-trading-btn:hover {
    background: #2baf9a;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(38, 166, 154, 0.3);
  }

  /* Charts Section */
  .charts-section {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .chart-card, .activity-card {
    background: #1a1f2e;
    border-radius: 12px;
    border: 1px solid #2a2e39;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    overflow: hidden;
  }

  .chart-header {
    padding: 16px 20px;
    background: #252a3a;
    border-bottom: 1px solid #2a2e39;
  }

  .chart-header h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #d1d4dc;
  }

  .chart-content {
    padding: 20px;
    height: 200px;
    position: relative;
  }

  .allocation-chart .chart-content {
    height: 250px;
  }

  /* Activity Card */
  .activity-list {
    padding: 16px 0;
    max-height: 300px;
    overflow-y: auto;
  }

  .activity-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 20px;
    transition: background-color 0.2s ease;
  }

  .activity-item:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .activity-icon {
    font-size: 16px;
    width: 24px;
    text-align: center;
  }

  .activity-details {
    flex: 1;
  }

  .activity-main {
    font-size: 13px;
    font-weight: 600;
    color: #d1d4dc;
    margin-bottom: 2px;
  }

  .activity-sub {
    font-size: 11px;
    color: #848e9c;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
  }

  .activity-time {
    font-size: 11px;
    color: #848e9c;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
  }

  .no-activity {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 40px 20px;
    color: #848e9c;
    font-size: 14px;
  }

  .no-activity-icon {
    font-size: 32px;
    margin-bottom: 8px;
    opacity: 0.5;
  }

  /* Scrollbar styling */
  .table-body::-webkit-scrollbar,
  .activity-list::-webkit-scrollbar {
    width: 6px;
  }

  .table-body::-webkit-scrollbar-track,
  .activity-list::-webkit-scrollbar-track {
    background: #0f1419;
  }

  .table-body::-webkit-scrollbar-thumb,
  .activity-list::-webkit-scrollbar-thumb {
    background: #2a2e39;
    border-radius: 3px;
  }

  .table-body::-webkit-scrollbar-thumb:hover,
  .activity-list::-webkit-scrollbar-thumb:hover {
    background: #848e9c;
  }

  /* Responsive Design */
  @media (max-width: 1200px) {
    .main-content {
      grid-template-columns: 1fr;
    }
    
    .charts-section {
      grid-template-columns: 1fr 1fr;
      display: grid;
    }
  }

  @media (max-width: 768px) {
    .overview-grid {
      grid-template-columns: 1fr;
      padding: 16px;
    }
    
    .main-content {
      padding: 0 16px 16px;
    }
    
    .portfolio-header {
      padding: 16px;
      flex-direction: column;
      gap: 16px;
      align-items: flex-start;
    }
    
    .header-left {
      flex-direction: column;
      align-items: flex-start;
      gap: 12px;
    }
    
    .table-header,
    .table-row {
      grid-template-columns: 1.5fr 0.8fr 0.8fr 1fr 1fr;
      font-size: 11px;
    }
    
    .col-price:first-of-type,
    .col-percent {
      display: none;
    }
    
    .charts-section {
      grid-template-columns: 1fr;
    }
  }
</style>
